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
    """An oblique crustal block cut by a dipping normal fault: offset strata, a fault scarp,
    the hypocentre on the fault plane, the epicentre above it, P and S wavefronts, slip arrows
    and a seismograph recording the P then the larger S arrival."""
    a = Art("verm", seed=11)
    o = (18, 164)
    X0, X1, ZT, D = 0.0, 204.0, 96.0, 112.0
    dip = math.radians(60)
    FX = 92.0                       # fault trace on the surface
    S = 8.0                         # vertical throw: the hanging wall (right) has dropped

    def fx(z, top=ZT):              # x of the fault plane at height z
        return FX + (top - z) / math.tan(dip)

    layers = [  # (top z, kind, colour)
        (96, "soil", PAL["soil"]), (91, "sand", PAL["sand2"]), (77, "shale", PAL["rock1"]),
        (63, "lime", PAL["sand1"]), (45, "sand", mix(PAL["soil"], PAL["sand2"], .4)), (31, "base", PAL["rock"]),
    ]
    bounds = [l[0] for l in layers] + [0]

    a.shadow(o[0] + 150, o[1] + 4, 150, 9, op=.2)

    # ---- footwall (left block): front face strata
    for i, (zt, kind, col) in enumerate(layers):
        zb = bounds[i + 1]
        poly = [ob(X0, 0, zt, o), ob(fx(zt), 0, zt, o), ob(fx(zb), 0, zb, o), ob(X0, 0, zb, o)]
        a.path(P(poly, True), fill=litho(a, kind, col))
    # footwall top
    ft = [ob(X0, 0, ZT, o), ob(X0, D, ZT, o), ob(FX, D, ZT, o), ob(FX, 0, ZT, o)]
    a.path(P(ft, True), fill=a.lin([(0, "#eef0e3"), (1, PAL["sage2"])], 0, 0, 1, 1))
    # exposed fault scarp (the footwall's fault plane above the dropped hanging wall)
    sc = [ob(FX, 0, ZT, o), ob(FX, D, ZT, o), ob(fx(ZT - S), D, ZT - S, o), ob(fx(ZT - S), 0, ZT - S, o)]
    a.path(P(sc, True), fill=a.lin([(0, PAL["umber"]), (1, mix(PAL["umber"], PAL["ink"], .3))], 0, 0, 1, 0))
    for k in range(1, 9):   # slickenlines down the dip of the scarp
        y = D * k / 9
        a.path(P([ob(FX, y, ZT, o), ob(fx(ZT - S), y, ZT - S, o)]), cls="hw", so=.35)

    # ---- hanging wall (right block), every boundary lowered by the throw
    for i, (zt, kind, col) in enumerate(layers):
        zb = bounds[i + 1]
        t2, b2 = zt - S, max(zb - S, 0)
        if t2 <= 0:
            continue
        poly = [ob(fx(t2 + S, ZT) if False else fx(t2), 0, t2, o), ob(X1, 0, t2, o), ob(X1, 0, b2, o), ob(fx(b2), 0, b2, o)]
        a.path(P(poly, True), fill=litho(a, kind, col, dy=-S))
        # right face continues the strata, a shade darker
        rf = [ob(X1, 0, t2, o), ob(X1, D, t2, o), ob(X1, D, b2, o), ob(X1, 0, b2, o)]
        a.path(P(rf, True), fill=mix(col, PAL["ink"], .2))
        a.path(P([ob(X1, 0, t2, o), ob(X1, D, t2, o)]), cls="h", so=.35)
    # the slab below the lowest dropped layer on the hanging-wall side
    poly = [ob(fx(S), 0, S, o), ob(X1, 0, S, o), ob(X1, 0, 0, o), ob(fx(0), 0, 0, o)]
    a.path(P(poly, True), fill=litho(a, "base", PAL["rock"], dy=-S))
    rf = [ob(X1, 0, S, o), ob(X1, D, S, o), ob(X1, D, 0, o), ob(X1, 0, 0, o)]
    a.path(P(rf, True), fill=mix(PAL["rock"], PAL["ink"], .2))
    ht = [ob(fx(ZT - S), 0, ZT - S, o), ob(fx(ZT - S), D, ZT - S, o), ob(X1, D, ZT - S, o), ob(X1, 0, ZT - S, o)]
    a.path(P(ht, True), fill=a.lin([(0, "#eef0e3"), (1, PAL["sage2"])], 0, 0, 1, 1))
    # grassland texture and a few trees on the surface (scale)
    rnd = a.rng
    gr = []
    for _ in range(240):
        x, y = rnd.uniform(X0 + 2, X1 - 2), rnd.uniform(2, D - 2)
        if abs(x - FX) < 4 or abs(x - fx(ZT - S)) < 3 or FX < x < fx(ZT - S):
            continue
        gr.append(ob(x, y, ZT if x < FX else ZT - S, o))
    a.dots(gr, r=.38, color=PAL["forest"], op=.45)
    for (x, y) in ((22, 78), (34, 92), (48, 70), (150, 92), (166, 80), (180, 98), (140, 104)):
        z = ZT if x < FX else ZT - S
        b = ob(x, y, z, o)
        a.path(conifer(b[0], b[1], 9), fill=PAL["forest"], stroke=PAL["forestd"], sw=.4)
    # depth shading on the front face (light from top left: darker with depth)
    front = [ob(X0, 0, ZT, o), ob(X1, 0, ZT, o), ob(X1, 0, 0, o), ob(X0, 0, 0, o)]
    a.path(P(front, True), fill=a.lin([(0, PAL["ink"], 0), (1, PAL["ink"], .16)], 0, 0, 0, 1))

    # ---- fault on the front face
    a.path(P([ob(FX, 0, ZT, o), ob(fx(0), 0, 0, o)]), cls="o", sw=1.3)
    # slip arrows (half arrows either side of the plane)
    zm = 60
    ux, uy = 1 / math.tan(dip), -1.0
    L = math.hypot(ux, uy)
    ux, uy = ux / L, uy / L
    pm = ob(fx(zm), 0, zm, o)
    off = 6
    nx, ny = -uy, ux                # normal pointing right/down-right
    p1 = (pm[0] - nx * off, pm[1] - ny * off)
    a.arrow([(p1[0] + ux * 9, p1[1] + uy * 9), (p1[0] - ux * 9, p1[1] - uy * 9)], w=1.0, head=4.6, half=True, curve=False)
    p2 = (pm[0] + nx * off, pm[1] + ny * off)
    a.arrow([(p2[0] - ux * 9, p2[1] - uy * 9), (p2[0] + ux * 9, p2[1] + uy * 9)], w=1.0, head=4.6, half=True, curve=False)

    # ---- hypocentre, wavefronts and epicentre
    zh = 24
    hyp = ob(fx(zh), 0, zh, o)
    front_clip = a.clip(P([ob(X0, 0, ZT, o), ob(FX, 0, ZT, o), ob(fx(ZT - S), 0, ZT - S, o), ob(X1, 0, ZT - S, o),
                           ob(X1, 0, 0, o), ob(X0, 0, 0, o)], True))
    a.group(cp=front_clip)
    for r, op in ((22, .95), (40, .75), (58, .55), (76, .35)):       # P fronts
        a.circle(hyp, r, fill="none", stroke=a.A, sw=1.0, so=op)
    for r, op in ((12.7, .9), (23.1, .7), (33.5, .5), (43.9, .35)):  # S fronts, r_P / 1.73
        a.circle(hyp, r, fill="none", stroke=a.A, sw=.8, da="2.2 1.8", so=op)
    a.end()
    # epicentre: vertically above the hypocentre on the dropped surface
    epx = fx(zh) - X0
    a.path(P([hyp, ob(epx, 0, ZT - S, o)]), cls="h", da="1.6 1.6", so=.8)
    ep = ob(epx, 0, ZT - S, o)
    # ground-motion rings around the epicentre on the surface (half rings into the block)
    for r, op in ((10, .9), (20, .6), (30, .38)):
        pts = []
        for k in range(0, 49):
            t = math.pi * k / 48
            x, y = epx + r * math.cos(t), r * math.sin(t) * 1.0
            if X0 < x < X1 and 0 <= y <= D and x > fx(ZT - S):
                pts.append(ob(x, y, ZT - S, o))
        if len(pts) > 2:
            a.path(smooth(pts), fill="none", stroke=a.A, sw=.8, so=op)
    a.path(_star(ep, 4.2, 1.8, 5), fill=a.A, stroke=PAL["white"], sw=.5)
    # hypocentre burst
    a.circle(hyp, 5.2, fill=a.AP, op=.75)
    a.path(_star(hyp, 4.6, 1.9, 8), fill=a.A, stroke=PAL["white"], sw=.5)

    # ---- outlines
    hw = ob(fx(ZT - S), 0, ZT - S, o)
    a.path(P([ob(X0, 0, 0, o), ob(X0, 0, ZT, o), ob(FX, 0, ZT, o)]) + P([hw, ob(X1, 0, ZT - S, o), ob(X1, 0, 0, o), ob(X0, 0, 0, o)]), cls="o")
    a.path(P([ob(X0, 0, ZT, o), ob(X0, D, ZT, o), ob(FX, D, ZT, o), ob(fx(ZT - S), D, ZT - S, o), ob(X1, D, ZT - S, o),
              ob(X1, D, 0, o), ob(X1, 0, 0, o)]) + P([ob(X1, 0, ZT - S, o), ob(X1, D, ZT - S, o)]) +
           P([ob(FX, 0, ZT, o), ob(FX, D, ZT, o)]) + P([hw, ob(fx(ZT - S), D, ZT - S, o)]), cls="o")

    # ---- seismograph station on the footwall and its record above
    st = ob(30, 40, ZT, o)
    a.path(P([(st[0], st[1] - 5), (st[0] + 3.4, st[1] + 1), (st[0] - 3.4, st[1] + 1)], True), fill=PAL["ink"])
    a.path(P([hyp, ob(30, 0, ZT, o)]), cls="h", da="0.8 1.6", so=.6)     # ray path to the station
    a.path(P([(st[0], st[1] - 6), (st[0], 33)]), cls="h", so=.5)
    # seismogram: quiet, P arrival, then the larger S arrival and coda
    x0, y0 = 14, 26
    rnd = a.rng
    pts = []
    for k in range(0, 156):
        x = x0 + k * 0.62
        if k < 30:
            amp = .4
        elif k < 70:
            amp = 3.2 * math.exp(-(k - 30) / 22)
        elif k < 156:
            amp = 9.5 * math.exp(-(k - 70) / 26) + .4
        y = y0 + amp * math.sin(k * 1.9) * (0.65 + 0.35 * rnd.random())
        pts.append((x, y))
    a.path(P(pts), fill="none", stroke=PAL["ink"], sw=.6, lj="round")
    for k, col in ((30, PAL["ink"]), (70, a.A)):                     # arrival ticks
        x = x0 + k * 0.62
        a.path(P([(x, 12), (x, 15.5)]), stroke=col, sw=.9)
    return a.render()


def _star(c, r1, r2, k=5, rot=-90):
    pts = []
    for i in range(2 * k):
        r = r1 if i % 2 == 0 else r2
        t = math.radians(rot + 180 * i / k)
        pts.append((c[0] + r * math.cos(t), c[1] + r * math.sin(t)))
    return P(pts, True)


def tornadoes():
    """A classic supercell seen from the south-east: anvil and overshooting top, the rotating
    mesocyclone with its striated updraft base, the rain-free base, the wall cloud, condensation
    funnel and debris cloud, warm inflow, the rear-flank downdraft, the forward-flank rain shaft
    and the damage track the tornado leaves across the fields."""
    a = Art("verm", seed=23)
    o = (6, 177)
    GW, GD, TH = 220.0, 100.0, 6.0
    # ---- ground slab of fields
    a.shadow(o[0] + 150, o[1] + 2, 150, 6, op=.18)
    fields_c = [PAL["sage1"], PAL["sage2"], PAL["sand1"], mix(PAL["sage1"], PAL["sand2"], .5), mix(PAL["sage2"], PAL["moss"], .3)]
    nx, ny = 9, 5
    for j in range(ny):
        for i in range(nx):
            x0, x1 = GW * i / nx, GW * (i + 1) / nx
            y0, y1 = GD * j / ny, GD * (j + 1) / ny
            q = [ob(x0, y0, 0, o), ob(x0, y1, 0, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)]
            a.path(P(q, True), fill=fields_c[(i * 3 + j * 2 + i * j) % 5])
            if (i + 2 * j) % 3 == 0:     # crop rows in some fields
                rows = "".join(P([ob(x0 + (x1 - x0) * t / 6, y0, 0, o), ob(x0 + (x1 - x0) * t / 6, y1, 0, o)]) for t in range(1, 6))
                a.path(rows, cls="h", so=.22)
    grid = "".join(P([ob(GW * i / nx, 0, 0, o), ob(GW * i / nx, GD, 0, o)]) for i in range(1, nx))
    grid += "".join(P([ob(0, GD * j / ny, 0, o), ob(GW, GD * j / ny, 0, o)]) for j in range(1, ny))
    a.path(grid, stroke=PAL["white"], sw=.9, so=.8, fill="none")       # section roads
    a.path(P([ob(0, 0, 0, o), ob(GW, 0, 0, o), ob(GW, 0, -TH, o), ob(0, 0, -TH, o)], True), fill=PAL["soil"])
    a.path(P([ob(GW, 0, 0, o), ob(GW, GD, 0, o), ob(GW, GD, -TH, o), ob(GW, 0, -TH, o)], True), fill=PAL["umber"])
    a.path(P([ob(0, 0, -TH, o), ob(0, 0, 0, o), ob(0, GD, 0, o), ob(GW, GD, 0, o), ob(GW, GD, -TH, o), ob(GW, 0, -TH, o), ob(0, 0, -TH, o)])
           + P([ob(0, 0, 0, o), ob(GW, 0, 0, o), ob(GW, GD, 0, o)]), cls="o")

    tg = (112.0, 40.0)                       # tornado position on the ground (x, depth)
    T = ob(tg[0], tg[1], 0, o)
    # damage track behind it (storm motion is towards the north-east: right and away)
    tr = [ob(10, 16, 0, o), ob(40, 22, 0, o), ob(76, 32, 0, o), T]
    a.path(smooth(tr), fill="none", stroke=a.AP, sw=7, lc="round")
    a.path(smooth(tr), fill="none", stroke=a.A, sw=1.6, lc="round", da="3 1.6")

    # ---- forward-flank precipitation: rain falling from under the anvil, behind and to the right
    g0, g1 = ob(150, 96, 0, o), ob(GW - 4, 84, 0, o)
    shaft = [(186, 101), (226, 97), (268, 99), (284, 112), g1, lerp2(g0, g1, .5), g0, (176, 120)]
    sp = smooth(shaft, True)
    a.path(sp, fill=a.lin([(0, PAL["slate"], .55), (.7, PAL["slate1"], .35), (1, PAL["slate1"], .12)], 0, 0, 0, 1))
    streaks = []
    for k in range(34):
        t0 = k / 33
        top = (lerp(184, 280, t0), lerp(102, 104, t0) + a.rng.random() * 4)
        bot = lerp2(g0, g1, t0)
        ln = .55 + .4 * a.rng.random()
        streaks.append(P([top, (lerp(top[0], bot[0] - 6, ln), lerp(top[1], bot[1], ln))]))
    a.path("".join(streaks), stroke=PAL["slate2"], sw=.5, so=.55, fill="none")
    # dark forward-flank base above the rain
    a.path(smooth([(170, 102), (220, 95), (280, 97), (292, 104), (240, 106), (186, 106)], True), fill=PAL["slate2"])

    # ---- anvil (sheared downwind to the right) and overshooting top
    anvil = smooth([(66, 36), (104, 24), (150, 17), (210, 18), (270, 20), (312, 25), (314, 31), (270, 37),
                    (210, 42), (160, 46), (112, 47), (72, 43)], True)
    a.path(anvil, fill=a.lin([(0, "#ffffff"), (1, PAL["cloud2"])], 0, 0, .25, 1))
    a.path(anvil, cls="h", so=.5)
    for k in range(3):
        a.path(smooth([(196 + k * 10, 26 + k * 4), (250, 25 + k * 4), (300, 27 + k * 2)]), cls="h", so=.18)
    ot = scallop([(118, 26), (126, 12), (140, 5), (156, 6), (168, 14), (172, 26)], bulge=.3, seed=3)
    a.path(ot, fill=a.lin([(0, "#ffffff"), (1, PAL["cloud"])], 0, 0, .6, 1))
    a.path(ot, cls="h", so=.5)

    # ---- the updraft tower: cauliflower silhouette, lit from the top left
    tw = [(84, 100), (78, 86), (82, 70), (90, 58), (100, 48), (116, 40), (134, 37), (152, 39), (168, 46),
          (180, 58), (186, 72), (186, 88), (182, 100)]
    tp = scallop(tw, bulge=.38, seed=9)
    a.path(tp, fill=a.lin([(0, "#ffffff"), (.5, PAL["cloud"]), (1, PAL["slate1"])], 0, 0, .8, 1))
    for (cx, cy, rx, ry) in ((112, 60, 16, 7), (150, 56, 18, 8), (126, 80, 22, 8), (164, 80, 14, 7)):
        a.path(smooth(ring_pts(cx, cy, rx, ry, 10, 180, 360)), fill="none", stroke="#fff", sw=1.2, so=.55)
        a.path(smooth(ring_pts(cx, cy + 1.2, rx, ry, 10, 200, 340)), fill="none", stroke=PAL["slate"], sw=.45, so=.35)
    a.path(tp, cls="h", so=.6)
    # striated, rotating updraft base ("mothership" bands)
    for k, (ry, w) in enumerate(((4.5, 50), (4.0, 44), (3.5, 37))):
        y = 92 + k * 3.4
        a.path(smooth(ring_pts(133, y, w, ry, 24, 10, 170)), fill="none", stroke=PAL["slate2"], sw=.55, so=.55)

    # ---- rain-free base and wall cloud
    base = smooth([(68, 104), (110, 100), (186, 100), (200, 106), (150, 110), (84, 110)], True)
    a.path(base, fill=a.lin([(0, PAL["slate2"]), (1, PAL["slated"])], 0, 0, 0, 1))
    wall = smooth([(110, 108), (154, 108), (150, 117), (142, 122), (122, 122), (114, 117)], True)
    a.path(wall, fill=a.lin([(0, PAL["slate2"]), (1, PAL["slated"])], 0, 0, 0, 1))
    for k, y in enumerate((111.5, 115, 118.5)):
        a.path(smooth(ring_pts(132, y, 19 - k * 2.6, 1.6, 12, 15, 165)), cls="hw", so=.35)

    # ---- condensation funnel and debris cloud
    fx0 = 132.0
    left = [(fx0 - 9, 121), (fx0 - 5.4, 132), (fx0 - 3.6, 144), (T[0] - 2.4, T[1] - 4)]
    right = [(T[0] + 2.6, T[1] - 4), (fx0 + 3.8, 144), (fx0 + 6.6, 132), (fx0 + 11, 121)]
    fun = smooth(left + right, False) + "Z"
    a.path(fun, fill=a.lin([(0, PAL["slate1"]), (.6, PAL["slate2"]), (1, PAL["slated"])], 0, 0, 1, 0))
    for k in range(9):
        t = k / 9
        y = lerp(124, T[1] - 6, t)
        cx = lerp(fx0 + 1, T[0], t)
        w = lerp(9.4, 3.0, t)
        a.path(f"M{n(cx - w)} {n(y)}q{n(w)} 1.7 {n(2 * w)} 0", cls="hw", so=.45)
    deb = scallop([(T[0] - 13, T[1] + 1), (T[0] - 10, T[1] - 7), (T[0] - 3, T[1] - 11), (T[0] + 5, T[1] - 10),
                   (T[0] + 12, T[1] - 5), (T[0] + 14, T[1] + 1)], bulge=.35, seed=12)
    a.path(deb, fill=a.lin([(0, PAL["rock"], .85), (1, PAL["umber"], .95)], 0, 0, 0, 1))
    a.dots([(T[0] - 16 + a.rng.random() * 32, T[1] - 15 + a.rng.random() * 14) for _ in range(30)], r=.42, color=PAL["char"])

    # ---- flows
    # mesocyclone: cyclonic (anticlockwise seen from above) rotation of the updraft
    pts = ring_pts(133, 82, 36, 7.5, 26, 205, 505)
    a.arrow(pts, color=a.A, w=1.3, head=5)
    # warm, moist inflow from the south-east, low down, then rising into the updraft
    a.arrow([(300, 176), (250, 170), (196, 158), (166, 138), (158, 104), (154, 66)], color=a.A, w=1.15, head=4.8)
    a.arrow([(312, 164), (262, 158), (214, 148), (184, 132)], color=a.A, w=.85, head=4, op=.75)
    # rear-flank downdraft descending behind the wall cloud and wrapping round the funnel
    a.arrow([(74, 60), (64, 92), (76, 128), (100, 150)], color=PAL["ink"], w=.9, head=4.2)
    return a.render()


def _flame(x, y, h, w, seed=0):
    """A single flame tongue, base centre at (x, y)."""
    return (f"M{n(x - w / 2)} {n(y)}C{n(x - w / 2)} {n(y - h * .45)} {n(x - w * .1)} {n(y - h * .6)} {n(x + w * .05)} {n(y - h)}"
            f"C{n(x + w * .2)} {n(y - h * .62)} {n(x + w / 2)} {n(y - h * .45)} {n(x + w / 2)} {n(y)}Z")


def fires():
    """A wildfire on forested hills: the burnt area behind the advancing fire front, the flame
    zone, a smoke plume bent over by the wind, and a polar-orbiting satellite whose scan picks
    out the hot 1 km pixels along the front (how FIRMS detects active fires)."""
    a = Art("verm", seed=31)
    o = (12, 158)
    Wd, D = 214.0, 100.0

    def h(x, y):
        return (14 * math.exp(-((x - 150) ** 2 / 2600 + (y - 70) ** 2 / 1500)) + 9 * math.exp(-((x - 60) ** 2 / 1800 + (y - 86) ** 2 / 900))
                + 3 * math.sin(x / 19) * math.cos(y / 23) + 4)

    def xf(y):                                   # fire front position across the block
        return 96 + 16 * math.sin(y / 26 + .6) + y * .12

    def cover(x, y):
        d = x - xf(y)
        if d < -5:
            return "burnt"
        if d < 2:
            return "front"
        return "forest"
    cols = {"burnt": mix(PAL["char"], PAL["rock"], .35), "front": a.A, "forest": PAL["forest"]}
    a.shadow(o[0] + 140, o[1] + 4, 150, 8, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=28, ny=12, cover=cover, colours=cols, base=12,
                   strata=[(3, PAL["umber"]), (7, PAL["clay"]), (99, PAL["rock"])])
    # trees on the unburnt slopes, back to front; standing dead snags in the burnt area
    rnd = a.rng
    trees, snags = [], []
    for j in range(13, 0, -1):
        for i in range(24):
            x, y = (i + .5 + rnd.uniform(-.35, .35)) * Wd / 24, (j - .5 + rnd.uniform(-.3, .3)) * D / 13
            if not (2 < x < Wd - 2 and 2 < y < D - 2):
                continue
            d = x - xf(y)
            if d > 4:
                trees.append(surf(x, y))
            elif d < -8 and rnd.random() < .45:
                snags.append(surf(x, y))
    a.path("".join(conifer(p[0], p[1] + .6, 6.4, 3.4) for p in trees), fill=PAL["forestd"], stroke=mix(PAL["forestd"], PAL["ink"], .3), sw=.3)
    a.path("".join(f"M{n(p[0])} {n(p[1])}v-4.2m-1.2 1.6l1.2-.6" for p in snags), stroke=PAL["ink"], sw=.5, fill="none", so=.7)
    # flame zone along the front: glow, then tongues
    fpts = [surf(xf(y), y) for y in [D * k / 40 for k in range(41)]]
    a.path(P(fpts), fill="none", stroke=a.AL, sw=7, lc="round", so=.45)
    for k, p in enumerate(fpts[1:-1:2]):
        hgt = 10 + 4 * math.sin(k * 1.7) + rnd.random() * 3
        a.path(_flame(p[0] + 1, p[1] + 1, hgt, 5.4), fill=a.A)
        a.path(_flame(p[0] + 1, p[1] + 1, hgt * .55, 2.8), fill=a.AL)
    # smoke plume rising from the front and bent over downwind (to the right)
    src = fpts[20]
    cl = [(lerp(src[0] + 2, 306, t), lerp(src[1] - 3, 30, 1 - (1 - t) ** 2.2)) for t in [k / 10 for k in range(11)]]
    wl = [lerp(6, 22, t ** .8) for t in [k / 10 for k in range(11)]]
    left, right = [], []
    for k in range(11):
        p, q = cl[max(0, k - 1)], cl[min(10, k + 1)]
        dx, dy = q[0] - p[0], q[1] - p[1]
        L = math.hypot(dx, dy)
        nx, ny = -dy / L, dx / L
        left.append((cl[k][0] - nx * wl[k], cl[k][1] - ny * wl[k]))
        right.append((cl[k][0] + nx * wl[k] * .8, cl[k][1] + ny * wl[k] * .8))
    pl = scallop(left[1:] + right[::-1][:-1], bulge=.16, seed=8, jitter=.4)
    a.path(pl, fill=a.lin([(0, PAL["slate2"], .8), (.5, PAL["slate1"], .7), (1, PAL["cloud2"], .55)], 0, 1, 1, 0))
    for k in range(2, 10, 2):    # billows, lit from the top left
        p = left[k]
        a.path(f"M{n(p[0] + 2)} {n(p[1] + 4)}q{n(wl[k] * .5)} {n(-wl[k] * .3)} {n(wl[k])} {n(wl[k] * .2)}", stroke="#fff", sw=.8, so=.5, fill="none")
    a.path(pl, cls="h", so=.35)
    # wind
    for k, y in enumerate((52, 62)):
        a.arrow([(18 + k * 6, y), (52 + k * 6, y - 2)], w=.9, head=4.2, curve=False)
    # satellite and its scan of the hot pixels
    sat = (54, 22)
    a.path(f"M{n(sat[0] - 11)} {n(sat[1] - 3)}h8v6h-8ZM{n(sat[0] + 3)} {n(sat[1] - 3)}h8v6h-8Z", fill=PAL["slate2"], stroke=PAL["ink"], sw=.4)
    a.path(f"M{n(sat[0] - 11)} {n(sat[1])}h22", stroke=PAL["ink"], sw=.35)
    a.path(f"M{n(sat[0] - 3)} {n(sat[1] - 4)}h6v8h-6Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.5)
    px = [(xf(y) + dx, y) for y, dx in ((22, -2), (34, 2), (46, -1), (58, 3), (70, 0), (82, 2))]
    for (x, y) in px:
        q = [surf(x - 5, y - 5, .3), surf(x - 5, y + 5, .3), surf(x + 5, y + 5, .3), surf(x + 5, y - 5, .3)]
        a.path(P(q, True), fill=a.AP, fo=.35, stroke=a.AD, sw=.6)
    lo, hi = surf(*px[0]), surf(*px[-1])
    a.path(P([(sat[0] - 2, sat[1] + 4), (lo[0] - 10, lo[1] + 3), (hi[0] + 12, hi[1] - 3), (sat[0] + 2, sat[1] + 4)], True),
           fill=a.AP, fo=.18, stroke=a.AD, sw=.4, so=.5, da="1.6 1.4")
    return a.render()
