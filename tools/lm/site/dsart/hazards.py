"""Earth and hazards: earthquakes, fires, tornadoes."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, cloud_path, conifer, scallop, ring_pts, terrain, HAIR)


def litho(art: Art, kind: str, base: str, dy: float = 0.0) -> str:
    """Geological lithology fills with the usual map conventions."""
    ink = mix(base, PAL["ink"], 0.45)
    tf = f"translate(0 {n(dy)})" if dy else None
    if kind == "sand":      # sandstone: dots
        c = (f'<circle cx="1" cy="1" r=".42" fill="{ink}"/><circle cx="3.5" cy="3.2" r=".42" fill="{ink}"/>')
        return art.pattern(5, 4.4, c, tf, base)
    if kind == "shale":     # shale: short dashes
        c = (f'<path d="M.5 1h3M5 3.4h3" stroke="{ink}" stroke-width=".45"/>')
        return art.pattern(5, 4.8, c, tf, base)
    if kind == "lime":      # limestone: brick courses
        c = (f'<path d="M0 .2h8M0 3.2h8M2 .2v3M6 3.2v3" stroke="{ink}" stroke-width=".42" fill="none"/>')
        return art.pattern(8, 6, c, tf, base)
    if kind == "base":      # crystalline basement: crosses
        c = (f'<path d="M1 2h2M2 1v2M5.5 5h2M6.5 4v2" stroke="{ink}" stroke-width=".42"/>')
        return art.pattern(8, 6, c, tf, base)
    return base


def earthquakes():
    """An oblique crustal block cut by a dipping normal fault: offset strata and a fault scarp,
    slip arrows on the fault, the hypocentre on the fault plane with seismic wavefronts spreading
    from it, and the epicentre on the surface directly above."""
    a = Art("verm", seed=11)
    o = (22, 160)
    X0, X1, ZT, D = 0.0, 204.0, 96.0, 112.0
    dip = math.radians(60)
    FX = 92.0                       # fault trace on the surface
    S = 10.0                        # vertical throw: the hanging wall (right) has dropped
    ZH = ZT - S                     # hanging-wall surface

    def fx(z):                      # x of the fault plane at height z (dips under the right block)
        return FX + (ZT - z) / math.tan(dip)

    layers = [(96, PAL["sand2"]), (76, PAL["clay"]), (54, PAL["rock1"]), (30, PAL["rock"])]
    bounds = [l[0] for l in layers] + [0]

    a.shadow(o[0] + 146, o[1] + 4, 132, 9, op=.2)

    # ---- footwall (left block) front face: strata cut by the fault
    for i, (zt, col) in enumerate(layers):
        zb = bounds[i + 1]
        a.path(P([ob(X0, 0, zt, o), ob(fx(zt), 0, zt, o), ob(fx(zb), 0, zb, o), ob(X0, 0, zb, o)], True), fill=col)
    # ---- hanging wall (right block): the same strata, every boundary lowered by the throw
    for i, (zt, col) in enumerate(layers):
        t2, b2 = zt - S, (bounds[i + 1] - S if i + 1 < len(layers) else 0)
        b2 = max(b2, 0)
        a.path(P([ob(fx(t2), 0, t2, o), ob(X1, 0, t2, o), ob(X1, 0, b2, o), ob(fx(b2), 0, b2, o)], True), fill=col)
        a.path(P([ob(X1, 0, t2, o), ob(X1, D, t2, o), ob(X1, D, b2, o), ob(X1, 0, b2, o)], True),
               fill=mix(col, PAL["ink"], .2))
        if i:
            a.path(P([ob(X1, 0, t2, o), ob(X1, D, t2, o)]), cls="h", so=.3)
    # depth shading on the front face (light from top left: darker with depth)
    front = P([ob(X0, 0, ZT, o), ob(FX, 0, ZT, o), ob(fx(ZH), 0, ZH, o), ob(X1, 0, ZH, o),
               ob(X1, 0, 0, o), ob(X0, 0, 0, o)], True)
    a.path(front, fill=a.lin([(0, PAL["ink"], 0), (1, PAL["ink"], .14)], 0, 0, 0, 1))

    # ---- ground surfaces and the exposed fault scarp between them
    grass = a.lin([(0, "#eef0e3"), (1, PAL["sage2"])], 0, 0, 1, 1)
    a.path(P([ob(X0, 0, ZT, o), ob(X0, D, ZT, o), ob(FX, D, ZT, o), ob(FX, 0, ZT, o)], True), fill=grass)
    a.path(P([ob(FX, 0, ZT, o), ob(FX, D, ZT, o), ob(fx(ZH), D, ZH, o), ob(fx(ZH), 0, ZH, o)], True),
           fill=a.lin([(0, PAL["umber"]), (1, mix(PAL["umber"], PAL["ink"], .3))], 0, 0, 1, 0))
    a.path(P([ob(fx(ZH), 0, ZH, o), ob(fx(ZH), D, ZH, o), ob(X1, D, ZH, o), ob(X1, 0, ZH, o)], True), fill=grass)

    # ---- hypocentre on the fault plane; wavefronts spread through the rock (front face only)
    zh = 26
    hyp = ob(fx(zh), 0, zh, o)
    a.group(cp=a.clip(front))
    for r, op in ((16, .95), (34, .65), (54, .38)):
        a.circle(hyp, r, fill="none", stroke=a.A, sw=1.2, so=op)
    a.end()

    # ---- fault on the front face and slip arrows: hanging wall down, footwall up (normal fault)
    a.path(P([ob(FX, 0, ZT, o), ob(fx(.7), 0, .7, o)]), cls="o", sw=1.4)   # stop short of the round cap
    zm = 62
    ux, uy = -1 / math.tan(dip), -1.0         # screen direction up-dip (toward the footwall top)
    L = math.hypot(ux, uy)
    ux, uy = ux / L, uy / L
    nx, ny = -uy, ux                          # normal pointing toward the hanging wall (right)
    pm = ob(fx(zm), 0, zm, o)
    off, h = 8.5, 12
    fw = (pm[0] - nx * off, pm[1] - ny * off)
    a.arrow([(fw[0] - ux * h, fw[1] - uy * h), (fw[0] + ux * h, fw[1] + uy * h)], w=1.6, head=7, curve=False)
    hw = (pm[0] + nx * off, pm[1] + ny * off)
    a.arrow([(hw[0] + ux * h, hw[1] + uy * h), (hw[0] - ux * h, hw[1] - uy * h)], w=1.6, head=7, curve=False)

    # ---- epicentre: vertically above the hypocentre on the dropped surface, with shaking rings
    epx = fx(zh)
    ep = ob(epx, 0, ZH, o)
    a.path(P([hyp, ep]), cls="h", da="2 1.8", so=.9)
    for r, op in ((11, .9), (22, .65)):
        pts = [ob(epx + r * math.cos(math.pi * k / 40), r * math.sin(math.pi * k / 40), ZH, o) for k in range(41)]
        a.path(smooth(pts), fill="none", stroke=a.A, sw=1.3, so=op)
    a.path(_star(ep, 6.5, 2.7, 5), fill=a.A, stroke=PAL["white"], sw=.7)
    a.circle(hyp, 6, fill=a.AP, op=.85)
    a.path(_star(hyp, 5.2, 2.2, 8), fill=a.A, stroke=PAL["white"], sw=.6)

    # ---- outlines
    hwc = ob(fx(ZH), 0, ZH, o)
    a.path(P([ob(X0, 0, 0, o), ob(X0, 0, ZT, o), ob(FX, 0, ZT, o)]) +
           P([hwc, ob(X1, 0, ZH, o), ob(X1, 0, 0, o), ob(X0, 0, 0, o)]), cls="o")
    a.path(P([ob(X0, 0, ZT, o), ob(X0, D, ZT, o), ob(FX, D, ZT, o), ob(fx(ZH), D, ZH, o), ob(X1, D, ZH, o),
              ob(X1, D, 0, o), ob(X1, 0, 0, o)]) + P([ob(X1, 0, ZH, o), ob(X1, D, ZH, o)]) +
           P([ob(FX, 0, ZT, o), ob(FX, D, ZT, o)]) + P([hwc, ob(fx(ZH), D, ZH, o)]), cls="o")
    return a.render()


def _star(c, r1, r2, k=5, rot=-90):
    pts = []
    for i in range(2 * k):
        r = r1 if i % 2 == 0 else r2
        t = math.radians(rot + 180 * i / k)
        pts.append((c[0] + r * math.cos(t), c[1] + r * math.sin(t)))
    return P(pts, True)


def _tornadoes_lobes(pts, bulge, seed):
    """Quadratic cumulus lobes along an open run of points (clockwise on screen: bulge outward)."""
    import random
    r = random.Random(seed)
    d = ""
    for p, q in zip(pts, pts[1:]):
        dx, dy = q[0] - p[0], q[1] - p[1]
        L = math.hypot(dx, dy)
        k = bulge * (.8 + .4 * r.random())
        c = ((p[0] + q[0]) / 2 + dy / L * L * k, (p[1] + q[1]) / 2 - dx / L * L * k)
        d += f"Q{n(c[0])} {n(c[1])} {n(q[0])} {n(q[1])}"
    return d


def _tornadoes_cr(pts, t=.5):
    """Absolute Catmull-Rom cubic segments through an open run of points."""
    d = ""
    m = len(pts)
    for i in range(m - 1):
        p0, p1, p2 = pts[max(i - 1, 0)], pts[i], pts[i + 1]
        p3 = pts[min(i + 2, m - 1)]
        c1 = (p1[0] + (p2[0] - p0[0]) * t / 3, p1[1] + (p2[1] - p0[1]) * t / 3)
        c2 = (p2[0] - (p3[0] - p1[0]) * t / 3, p2[1] - (p3[1] - p1[1]) * t / 3)
        d += f"C{n(c1[0])} {n(c1[1])} {n(c2[0])} {n(c2[1])} {n(p2[0])} {n(p2[1])}"
    return d


def tornadoes():
    """A supercell seen from the south-east: the anvil sheared downwind, the rotating updraft
    (mesocyclone) fed by warm inflow, the wall cloud and its tornado, and the damage track the
    tornado leaves across the fields as the storm moves north-east."""
    a = Art("verm", seed=23)
    o = (14, 168)
    GW, GD, TH = 200.0, 88.0, 6.0
    # ---- ground slab of fields
    a.shadow(o[0] + 150, o[1] + TH + 1, 140, 5, op=.16)
    fields_c = [PAL["sage1"], PAL["sage2"], PAL["sand1"]]
    nx, ny = 5, 3
    for j in range(ny):
        for i in range(nx):
            x0, x1 = GW * i / nx, GW * (i + 1) / nx
            y0, y1 = GD * j / ny, GD * (j + 1) / ny
            q = [ob(x0, y0, 0, o), ob(x0, y1, 0, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)]
            a.path(P(q, True), fill=fields_c[(i + 2 * j) % 3])
    grid = "".join(P([ob(GW * i / nx, 0, 0, o), ob(GW * i / nx, GD, 0, o)]) for i in range(1, nx))
    grid += "".join(P([ob(0, GD * j / ny, 0, o), ob(GW, GD * j / ny, 0, o)]) for j in range(1, ny))
    a.path(grid, stroke=PAL["white"], sw=1.1, so=.85, fill="none")       # section roads
    a.path(P([ob(0, 0, 0, o), ob(GW, 0, 0, o), ob(GW, 0, -TH, o), ob(0, 0, -TH, o)], True), fill=PAL["soil"])
    a.path(P([ob(GW, 0, 0, o), ob(GW, GD, 0, o), ob(GW, GD, -TH, o), ob(GW, 0, -TH, o)], True), fill=PAL["umber"])
    a.path(P([ob(0, 0, -TH, o), ob(0, 0, 0, o), ob(0, GD, 0, o), ob(GW, GD, 0, o), ob(GW, GD, -TH, o), ob(GW, 0, -TH, o), ob(0, 0, -TH, o)])
           + P([ob(0, 0, 0, o), ob(GW, 0, 0, o), ob(GW, GD, 0, o)]), cls="o")

    tg = (98.0, 34.0)                        # tornado position on the ground (x, depth)
    T = ob(tg[0], tg[1], 0, o)
    # damage track behind it: the storm moves north-east (right and away), so the track
    # runs back towards the south-west (left and nearer)
    tr = [ob(6, 10, 0, o), ob(36, 16, 0, o), ob(68, 25, 0, o), T]
    a.path(smooth(tr), fill="none", stroke=a.AP, sw=7, lc="round")
    a.path(smooth(tr), fill="none", stroke=a.A, sw=1.6, lc="round", da="3 1.6")

    cx = T[0] + 2                            # storm axis on screen
    # ---- the supercell as ONE silhouette: a cauliflower updraft tower that spreads into the
    # anvil (flat top sheared downwind to the right, overshooting dome above the updraft) and
    # ends below in a flat rain-free base
    BY = 103.0                               # flat cloud base
    left = [(cx - 50, BY), (cx - 54, 90), (cx - 50, 76), (cx - 42, 64), (cx - 36, 53)]
    anvil_top = [(cx - 36, 53), (cx - 50, 44), (cx - 72, 37), (cx - 76, 32), (cx - 58, 27), (cx - 24, 24)]
    dome = [(cx - 24, 24), (cx - 18, 14), (cx - 6, 8), (cx + 8, 8), (cx + 20, 13), (cx + 27, 22)]
    flat = [(cx + 27, 22), (cx + 80, 21), (cx + 140, 23), (300, 27), (302, 31), (cx + 140, 35),
            (cx + 80, 38), (cx + 50, 42), (cx + 38, 52)]
    right = [(cx + 38, 52), (cx + 48, 63), (cx + 54, 77), (cx + 54, 91), (cx + 52, BY)]
    sil = (f"M{n(left[0][0])} {n(left[0][1])}" + _tornadoes_lobes(left, .36, 5)
           + _tornadoes_cr(anvil_top) + _tornadoes_lobes(dome, .3, 3) + _tornadoes_cr(flat)
           + _tornadoes_lobes(right, .36, 7) + "Z")
    a.path(sil, fill=a.lin([(0, "#ffffff"), (.55, PAL["cloud"]), (1, PAL["slate1"])], 0, 0, .3, 1))
    a.group(cp=a.clip(sil))
    # the darker underside of the anvil, and the dark rain-free base under the updraft
    a.path(smooth([(cx + 30, 40), (cx + 90, 33), (cx + 150, 31), (306, 30), (306, 40), (cx + 30, 46)], True),
           fill=a.lin([(0, PAL["cloud2"], 0), (.35, PAL["cloud2"], .8), (1, PAL["cloud2"], .8)], 0, 0, 1, 0))
    a.path(f"M0 {n(BY - 7)}H320V{n(BY + 2)}H0Z", fill=a.lin([(0, PAL["slate2"], 0), (.5, PAL["slate2"]), (1, PAL["slated"])], 0, 0, 0, 1))
    for (ux, uy, rx, ry) in ((-20, 64, 15, 6), (16, 62, 17, 7)):
        a.path(smooth(ring_pts(cx + ux, uy, rx, ry, 10, 190, 350)), fill="none", stroke="#fff", sw=1.4, so=.7)
    a.end()
    a.path(sil, cls="o", so=.55)

    # ---- the lowered, rotating wall cloud hanging from the base beneath the updraft
    wall = smooth([(cx - 20, BY - 1), (cx + 22, BY - 1), (cx + 19, 111), (cx + 11, 118), (cx - 9, 118), (cx - 17, 111)], True)
    a.path(wall, fill=a.lin([(0, PAL["slate2"]), (1, PAL["slated"])], 0, 0, 0, 1))
    a.path(wall, cls="h", so=.5, cp=a.clip(f"M0 {n(BY + .6)}H320V180H0Z"))

    # ---- condensation funnel reaching the ground, and the debris cloud at its foot
    left = [(cx - 9, 116), (cx - 6, 130), (cx - 4, 141), (T[0] - 2.4, T[1] - 3)]
    right = [(T[0] + 2.6, T[1] - 3), (cx + 4, 141), (cx + 7, 130), (cx + 10, 116)]
    fun = smooth(left + right, False) + "Z"
    a.path(fun, fill=a.lin([(0, PAL["slate1"]), (.6, PAL["slate2"]), (1, PAL["slated"])], 0, 0, 1, 0))
    for k in range(4):
        t = (k + .5) / 4
        y = lerp(123, T[1] - 6, t)
        xm = lerp(cx + .5, T[0], t)
        w = lerp(8.6, 3.2, t)
        a.path(f"M{n(xm - w)} {n(y)}q{n(w)} 1.8 {n(2 * w)} 0", cls="hw", so=.6)
    deb = scallop([(T[0] - 12, T[1] + 1.5), (T[0] - 9, T[1] - 6), (T[0] - 2, T[1] - 10), (T[0] + 5, T[1] - 9),
                   (T[0] + 11, T[1] - 4), (T[0] + 13, T[1] + 1.5)], bulge=.35, seed=12)
    a.path(deb, fill=a.lin([(0, PAL["rock"], .85), (1, PAL["umber"], .95)], 0, 0, 0, 1))
    a.dots([(T[0] - 14, T[1] - 9), (T[0] + 14, T[1] - 11), (T[0] - 9, T[1] - 15), (T[0] + 9, T[1] - 16),
            (T[0] + 17, T[1] - 5)], r=.8, color=PAL["char"])

    # ---- flows
    # warm, moist inflow from the south-east: low over the fields, then rising into the updraft
    a.arrow([ob(194, 6, 0, o), ob(172, 17, 0, o), ob(152, 27, 0, o), (cx + 41, 138), (cx + 34, BY + 3)], color=a.A, w=1.3, head=5)
    # mesocyclone: cyclonic rotation (anticlockwise seen from above), so the near side moves east
    pts = ring_pts(cx - 2, 82, 32, 7, 26, 200, -40)
    a.arrow(pts, color=a.A, w=1.3, head=5)
    return a.render()


def _flame(x, y, h, w, seed=0):
    """A single flame tongue, base centre at (x, y)."""
    return (f"M{n(x - w / 2)} {n(y)}C{n(x - w / 2)} {n(y - h * .45)} {n(x - w * .1)} {n(y - h * .6)} {n(x + w * .05)} {n(y - h)}"
            f"C{n(x + w * .2)} {n(y - h * .62)} {n(x + w / 2)} {n(y - h * .45)} {n(x + w / 2)} {n(y)}Z")


def _fires_tree(x, y, h):
    """A conifer standing on (x, y): crown plus a short trunk that reaches the ground."""
    w = h * .5
    return (f"M{n(x - .5)} {n(y)}v{n(-h * .2)}h1v{n(h * .2)}Z"
            f"M{n(x)} {n(y - h)}L{n(x + w * .34)} {n(y - h * .58)}L{n(x + w * .2)} {n(y - h * .58)}"
            f"L{n(x + w * .5)} {n(y - h * .16)}L{n(x - w * .5)} {n(y - h * .16)}"
            f"L{n(x - w * .2)} {n(y - h * .58)}L{n(x - w * .34)} {n(y - h * .58)}Z")


def fires():
    """A wildfire on forested hills: the burnt scar behind the advancing fire front, the flame
    line, a smoke plume bent over by the wind, and a polar-orbiting satellite whose scan flags
    the hot pixels along the front (how FIRMS detects active fires)."""
    a = Art("verm", seed=31)
    o = (12, 158)
    Wd, D = 214.0, 100.0

    def h(x, y):
        return (13 * math.exp(-((x - 150) ** 2 / 2600 + (y - 70) ** 2 / 1500)) + 8 * math.exp(-((x - 60) ** 2 / 1800 + (y - 86) ** 2 / 900)) + 4)

    def xf(y):                                   # fire front position across the block
        return 98 + 12 * math.sin(y / 30 + .6) + y * .1

    def cover(x, y):
        return "burnt" if x < xf(y) - 3 else "forest"
    cols = {"burnt": mix(PAL["char"], PAL["rock"], .4), "forest": PAL["forest"]}
    a.shadow(o[0] + 140, o[1] + 4, 150, 8, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=24, ny=10, cover=cover, colours=cols, base=12,
                   strata=[(4, PAL["umber"]), (99, PAL["rock"])])
    # satellite and its scan swath, drawn first so everything on the ground sits on top of it
    hp = 9                                       # half-width of a hot pixel, in block units
    pys = (18, 48, 78)
    px = [(xf(y), y) for y in pys]
    sq = [[surf(x - hp, y - hp, .3), surf(x - hp, y + hp, .3), surf(x + hp, y + hp, .3), surf(x + hp, y - hp, .3)] for (x, y) in px]
    sat = (54, 24)
    corners = [c for q in sq for c in q]
    ang = lambda c: math.atan2(c[1] - sat[1] - 5, c[0] - sat[0])
    c1, c2 = max(corners, key=ang), min(corners, key=ang)
    a.path(P([(sat[0] - 2, sat[1] + 5), c1, c2, (sat[0] + 2, sat[1] + 5)], True), fill=a.AP, fo=.28)
    for c, s in ((c1, -2), (c2, 2)):
        a.path(P([(sat[0] + s, sat[1] + 5), c]), stroke=a.AD, sw=.6, so=.6, da="2 1.6", fill="none")
    # hot pixels flagged by the satellite: square footprints on the ground along the front
    for q in sq:
        a.path(P(q, True), fill=a.AP, fo=.95, stroke=a.AD, sw=1.6, lj="round")
    # a thin glowing line marks the continuous fire front between and under the pixels
    a.path(P([surf(xf(y), y, .4) for y in [pys[0] + (pys[-1] - pys[0]) * k / 24 for k in range(25)]]),
           fill="none", stroke=a.A, sw=1.6, lc="round")
    # smoke plume: its base spans the rear half of the front, it widens as it rises and bends downwind
    yb0, yb1 = pys[1], pys[2]
    base = [surf(xf(y), y) for y in [yb0 + (yb1 - yb0) * k / 8 for k in range(9)]]
    base = [(x, y - 11) for (x, y) in base]  # at the flame tips
    A, B = base[0], base[-1]
    M = ((A[0] + B[0]) / 2, (A[1] + B[1]) / 2)
    hw = math.hypot(B[0] - A[0], B[1] - A[1]) / 2
    cl = [(M[0] + (296 - M[0]) * t ** 1.5, lerp(M[1], 34, 1 - (1 - t) ** 2)) for t in [k / 6 for k in range(7)]]
    wl = [lerp(hw, hw * 1.45, k / 6) for k in range(7)]
    left, right = [A], [B]
    for k in range(1, 7):
        p, q = cl[k - 1], cl[min(6, k + 1)]
        dx, dy = q[0] - p[0], q[1] - p[1]
        L = math.hypot(dx, dy)
        nx, ny = -dy / L, dx / L
        left.append((cl[k][0] - nx * wl[k], cl[k][1] - ny * wl[k]))
        right.append((cl[k][0] + nx * wl[k] * .8, cl[k][1] + ny * wl[k] * .8))
    pl = scallop(left + right[::-1] + base[-2:0:-1], bulge=.12, seed=8, jitter=.3)
    plume = lambda: (a.path(pl, fill=a.lin([(0, PAL["slate2"], .9), (.5, PAL["slate1"], .78), (1, PAL["cloud2"], .65)], 0, 1, 1, 0)), a.path(pl, cls="h", so=.35))
    # trees, burnt snags and flames, drawn back to front so nearer things overlap farther ones
    rnd = a.rng
    items = []
    for j in range(4):
        for i in range(9):
            x = (i + .5 + rnd.uniform(-.2, .2)) * Wd / 9
            y = (j + .5 + rnd.uniform(-.15, .15)) * D / 4
            d = x - xf(y)
            if d > 20 and x < Wd - 8:
                items.append((y, "tree", x))
            elif d < -16 and x > 8 and (i + j) % 2 == 0:
                items.append((y, "snag", x))
    for y in pys:                                # one flame centred in each hot pixel
        items.append((y, "flame", xf(y)))
    items.append((yb1 + 4, "plume", 0))          # only things behind the plume's base go under it
    for y, kind, x in sorted(items, key=lambda t: (-t[0], t[1] == "flame")):
        if kind == "plume":
            plume()
            continue
        p = surf(x, y)
        if kind == "snag" and p[0] > A[0] - 12 and p[1] < A[1] + 14:
            continue                             # keep the plume's foot clear
        if kind == "tree":
            a.path(_fires_tree(p[0], p[1] + .5, 13), fill=PAL["forestd"], stroke=mix(PAL["forestd"], PAL["ink"], .35), sw=.5)
        elif kind == "snag":
            a.path(f"M{n(p[0])} {n(p[1] + .5)}v-9m0 3.4l2.4-1.8m-2.4 4l-2-1.2", stroke=PAL["ink"], sw=1.1, fill="none", so=.75, lc="round")
        else:
            a.path(_flame(p[0], p[1] + 1, 14, 8), fill=a.A)
            a.path(_flame(p[0], p[1] + 1, 7.5, 4), fill=a.AL)
    # the satellite itself
    a.path(f"M{n(sat[0] - 15)} {n(sat[1] - 4)}h11v8h-11ZM{n(sat[0] + 4)} {n(sat[1] - 4)}h11v8h-11Z", fill=PAL["slate2"], stroke=PAL["ink"], sw=.6)
    a.path(f"M{n(sat[0] - 15)} {n(sat[1])}h30", stroke=PAL["ink"], sw=.5)
    a.path(f"M{n(sat[0] - 4)} {n(sat[1] - 5)}h8v10h-8Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.7)
    return a.render()
