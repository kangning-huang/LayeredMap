"""Energy and industry: wind turbines, nuclear reactors, solar farms, mines."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, scallop, ring_pts, terrain, draw_box,
                  box_faces, conifer)


def _slab(a: Art, o, Wd, D, th, top, front=None, right=None, fields=None):
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=top)
    if fields:
        fields()
    a.path(P(f, True), fill=front or PAL["soil"])
    a.path(P(r, True), fill=right or PAL["umber"])
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    return t


def _turbine(a: Art, base, hub_h, R, ang=8.0, tilt=.5, detail=True, colour=None):
    """A three-bladed horizontal-axis turbine; the rotor faces the wind from the front left, so
    its disc is an ellipse (x squashed by tilt). Returns the hub position."""
    bx, by = base
    hub = (bx, by - hub_h)
    w0, w1 = hub_h * .045, hub_h * .022
    c = colour or "#fbfbf9"
    a.path(P([(bx - w0, by), (bx - w1, hub[1] + 2), (bx + w1, hub[1] + 2), (bx + w0, by)], True),
           fill=a.lin([(0, c), (1, mix(c, PAL["ink"], .25))], 0, 0, 1, 0), stroke=PAL["ink"], sw=.45 if detail else .3)
    # nacelle extends downwind (behind the hub, to the right)
    nl, nh = R * .32, R * .1
    nac = [(hub[0] - nl * .1, hub[1] - nh * .5), (hub[0] + nl, hub[1] - nh * .55), (hub[0] + nl, hub[1] + nh * .5), (hub[0] - nl * .1, hub[1] + nh * .55)]
    a.path(P(nac, True), fill=a.lin([(0, c), (1, mix(c, PAL["ink"], .2))], 0, 0, 0, 1), stroke=PAL["ink"], sw=.5 if detail else .3)
    for k in range(3):
        t = math.radians(ang + 120 * k)
        d = (math.cos(t) * tilt, math.sin(t))
        nrm = (-d[1], d[0])
        tip = (hub[0] + d[0] * R, hub[1] + d[1] * R)
        root = R * .055
        chord = R * .07
        pts = [(hub[0] + nrm[0] * root, hub[1] + nrm[1] * root),
               (hub[0] + d[0] * R * .22 + nrm[0] * chord, hub[1] + d[1] * R * .22 + nrm[1] * chord),
               tip,
               (hub[0] + d[0] * R * .3 - nrm[0] * chord * .35, hub[1] + d[1] * R * .3 - nrm[1] * chord * .35),
               (hub[0] - nrm[0] * root, hub[1] - nrm[1] * root)]
        a.path(smooth(pts, True, t=.3), fill=a.lin([(0, "#fff"), (1, mix(c, PAL["ink"], .18))], 0, 0, 1, 1),
               stroke=PAL["ink"], sw=.45 if detail else .3, lj="round")
    a.ellipse(hub, R * .06 * tilt + 1, R * .06 + .6, fill=c, stroke=PAL["ink"], sw=.5 if detail else .3)
    return hub, nac


def _wind_turbines_turbine(a: Art, o, x, y, H, R, ang, colour="#fbfbf9", sw=.5, disc=False):
    """An upwind three-bladed turbine in the block's oblique projection. The wind blows along +x,
    so the rotor turns in the y-z plane (a sheared ellipse on screen) at the upwind (-x) end of the
    nacelle. The viewer stands on the +x side, so the rotor is drawn first and the tower and
    nacelle in front of it."""
    c = colour
    base = ob(x, y, 0, o)
    over = R * .12                       # rotor overhang upwind of the tower axis
    hx = x - over
    hub = ob(hx, y, H, o)

    def rim(t, r):                       # point on the rotor plane at angle t, radius r
        return ob(hx, y + math.cos(t) * r, H + math.sin(t) * r, o)

    if disc:
        a.path(P([rim(math.radians(k * 10), R) for k in range(36)], True), fill=a.AP, fo=.45,
               stroke=a.A, sw=.8, da="3 2")
    # blades: tapered, root at the hub, in the rotor plane
    for k in range(3):
        t = math.radians(ang + 120 * k)
        tip = rim(t, R)
        sx, sy = (rim(t + .5 * math.pi, R * .07)[0] - hub[0], rim(t + .5 * math.pi, R * .07)[1] - hub[1])
        mid = rim(t, R * .25)
        pts = [(hub[0] + sx * .6, hub[1] + sy * .6), (mid[0] + sx, mid[1] + sy), tip,
               (mid[0] - sx * .5, mid[1] - sy * .5), (hub[0] - sx * .6, hub[1] - sy * .6)]
        a.path(smooth(pts, True, t=.3), fill=a.lin([(0, "#fff"), (1, mix(c, PAL["ink"], .18))], 0, 0, 1, 1),
               stroke=PAL["ink"], sw=sw, lj="round")
    a.ellipse(hub, R * .05 + .6, R * .07 + .6, fill=c, stroke=PAL["ink"], sw=sw)
    # tower: a tapered tube from the ground to the nacelle floor
    top = ob(x, y, H - R * .05, o)
    w0, w1 = H * .045, H * .022
    a.path(P([(base[0] - w0, base[1]), (top[0] - w1, top[1]), (top[0] + w1, top[1]), (base[0] + w0, base[1])], True),
           fill=a.lin([(0, c), (1, mix(c, PAL["ink"], .25))], 0, 0, 1, 0), stroke=PAL["ink"], sw=sw)
    # nacelle: a small oblique box sitting on the tower top, from the hub back downwind
    nl, nh, nd = R * .42, R * .11, R * .09
    f, tp, r = box_faces(0, nl, -nh / 2, nh / 2, nd, ob(hx + R * .03, y - nd / 2, H, o))
    a.path(P(f, True), fill=c)
    a.path(P(tp, True), fill="#fff")
    a.path(P(r, True), fill=mix(c, PAL["ink"], .22))
    a.path(P(f, True) + P(tp, True) + P([r[1], r[2], r[3]]), fill="none", stroke=PAL["ink"], sw=sw, lj="round")
    return hub


def wind_turbines():
    """A wind farm on the plains: one large upwind turbine with its swept rotor disc, facing a wind
    that strengthens with height (the logarithmic profile), and a few turbines behind, every
    rotor turned into the same wind."""
    a = Art("cobalt", seed=51)
    o = (12, 170)
    Wd, D = 236.0, 84.0

    def fields():
        cols, rows = (0, 70, 150, Wd), (0, 46, D)
        tones = (PAL["sage1"], mix(PAL["sage1"], PAL["sage2"], .55), PAL["sand1"])
        for j in range(2):
            for i in range(3):
                q = [ob(cols[i], rows[j], 0, o), ob(cols[i], rows[j + 1], 0, o),
                     ob(cols[i + 1], rows[j + 1], 0, o), ob(cols[i + 1], rows[j], 0, o)]
                a.path(P(q, True), fill=tones[(i + 2 * j) % 3])
    a.shadow(o[0] + 150, o[1] + 2, 150, 5, op=.18)
    _slab(a, o, Wd, D, 5, PAL["sage1"], fields=fields)
    # the farm behind, back to front; all rotors face the same (left) wind
    for (x, y, ang) in ((205, 74, 20), (168, 40, 75), (146, 8, 50)):
        b = ob(x, y, 0, o)
        a.shadow(b[0] + 5, b[1] + .5, 6, 1.2, op=.25)
        _wind_turbines_turbine(a, o, x, y, 50, 27, ang, colour="#f4f4f1", sw=.45)
    # ---- the near turbine with its swept area
    xt, yt, Ht, Rt = 80.0, 26.0, 94.0, 50.0
    b = ob(xt, yt, 0, o)
    a.shadow(b[0] + 9, b[1] + 1, 13, 2.2, op=.3)
    _wind_turbines_turbine(a, o, xt, yt, Ht, Rt, -32, sw=.6, disc=True)
    # ---- wind profile on the ground at the left: speed grows with height (log law)
    px, py = 8.0, yt
    g = ob(px, py, 0, o)
    def L(z):
        return 9.5 * math.log(z / 3.0)
    for z in (10, 40, 72, 104):
        p0 = ob(px, py, z, o)
        a.arrow([p0, (p0[0] + L(z), p0[1])], color=a.A, w=1.1, head=4.4, curve=False)
    a.path(smooth([(g[0] + L(z), g[1] - z) for z in (3.5, 6, 10, 18, 30, 45, 62, 82, 104)]), cls="h", so=.8)
    a.path(P([g, ob(px, py, 108, o)]), cls="an")
    return a.render()


def _nuclear_hull(pts):
    """Convex hull (monotone chain) of screen points, counter-clockwise."""
    pts = sorted(set((round(x, 2), round(y, 2)) for x, y in pts))

    def half(seq):
        h = []
        for p in seq:
            while len(h) > 1 and ((h[-1][0] - h[-2][0]) * (p[1] - h[-2][1]) - (h[-1][1] - h[-2][1]) * (p[0] - h[-2][0])) <= 0:
                h.pop()
            h.append(p)
        return h[:-1]
    return half(pts) + half(pts[::-1])


def _nuclear_ring(o, cx, cy, z, r, a0=0, a1=360, k=36):
    """A horizontal circle (centre cx at depth cy, height z) in the block's oblique projection."""
    return [ob(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / k)),
               cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / k)), z, o) for i in range(k + 1)]


def _nuclear_spline(pts, k=8):
    """Points sampled along an open Catmull-Rom spline through pts (ends included)."""
    ext = [pts[0]] + list(pts) + [pts[-1]]
    out = []
    for i in range(1, len(ext) - 2):
        p0, p1, p2, p3 = ext[i - 1], ext[i], ext[i + 1], ext[i + 2]
        for j in range(k):
            t = j / k
            out.append(tuple(.5 * (2 * p1[c] + (p2[c] - p0[c]) * t + (2 * p0[c] - 5 * p1[c] + 4 * p2[c] - p3[c]) * t * t
                                   + (3 * p1[c] - p0[c] - 3 * p2[c] + p3[c]) * t ** 3) for c in (0, 1)))
    out.append(tuple(pts[-1]))
    return out


def nuclear():
    """A pressurised-water reactor, cut open along the block's front face like a textbook figure:
    in the containment the reactor vessel (glowing core, control rods) heats the primary loop,
    whose steam generator raises steam for the turbine and generator in the turbine hall; the
    condenser returns the water and sends its cooling water to the cooling tower, which vents a
    plume of condensed vapour."""
    from .kit import DX
    a = Art("verm", seed=53)
    o = (8, 168)
    DEP = 50

    def p(x, y=0.0, z=0.0):
        return ob(x, y, z, o)

    a.shadow(166, 172, 158, 7, op=.2)
    f, t, r = box_faces(0, 272, -6, 0, DEP, o)
    a.path(P(t, True), fill=PAL["sage1"])
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    steam, water, D = PAL["water3"], PAL["water4"], 12   # every pipe and machine axis runs at depth D
    hot, cold, ink = a.A, a.AD, PAL["ink"]

    def flow(x0, x1, z, d=D, size=9):
        a.head(p(x1, d, z), p(x0, d, z), ink, size)

    # ---- containment: a domed cylinder whose front half is cut away on the block's front face
    cx, R, Ri, Hc, Hd, Hdi = 44, 38, 33, 70, 30, 26
    shell_pts = []
    for i in range(19):
        th = math.radians(i * 10)
        for z, rr in [(0, R), (Hc, R)] + [(Hc + Hd * math.sin(math.radians(e)), R * math.cos(math.radians(e)))
                                          for e in range(15, 91, 15)]:
            shell_pts.append(p(cx + rr * math.cos(th), rr * math.sin(th), z))
    hull = P(_nuclear_hull(shell_pts), True)
    a.path(hull, fill=a.lin([(0, PAL["rock1"]), (1, PAL["rock"])], 0, 0, 1, .3))
    a.path(hull, cls="o")
    bx, by = p(cx, 0, 0)

    def cut(rr, hd):
        return f"M{n(bx - rr)} {n(by)}V{n(by - Hc)}A{rr} {hd} 0 0 1 {n(bx + rr)} {n(by - Hc)}V{n(by)}Z"
    outer, inner = cut(R, Hd), cut(Ri, Hdi)
    a.path(inner, fill=a.lin([(0, "#f7f5f0"), (1, PAL["paper2"])], 0, 0, 0, 1))
    a.group(cp=a.clip(inner))
    # the round floor of the cylinder, seen obliquely (its back half rises as a half-ellipse)
    a.path(P(_nuclear_ring(o, cx, 0, 0, Ri, 0, 180), True), fill=PAL["rock1"])
    # primary loop at depth D: hot leg up top, cold leg below, drawn first so the vessels cover the ends
    vx, sx = 26, 58
    a.path(P([p(vx, D, 40), p(sx, D, 40)]), stroke=hot, sw=3.6, fill="none")
    a.path(P([p(sx, D, 16), p(vx, D, 16)]), stroke=cold, sw=3.6, fill="none")
    # steam leaves the top of the steam generator, feedwater returns low on its side
    a.path(P([p(sx, D, 60), p(sx, D, 84), p(cx + 40, D, 84)]), stroke=steam, sw=3, fill="none", lj="round")
    a.path(P([p(sx, D, 22), p(cx + 40, D, 22)]), stroke=water, sw=2.4, fill="none")
    # reactor pressure vessel standing on the floor: glowing core, control rods from the head
    rx, ry = p(vx, D, 0)
    a.path(f"M{n(rx - 10)} {n(ry - 6)}V{n(ry - 46)}a10 6 0 0 1 20 0V{n(ry - 6)}a10 6 0 0 1-20 0Z",
           fill=PAL["slate1"], stroke=ink, sw=.9)
    a.path(f"M{n(rx - 7)} {n(ry - 10)}h14v-18h-14Z", fill=a.rad([(0, a.AP), (1, a.AL)]), stroke=a.AD, sw=.7)
    a.path("".join(f"M{n(rx - 4.5 + k * 4.5)} {n(ry - 52)}V{n(ry - 22 - (k % 2) * 4)}" for k in range(3)),
           stroke=ink, sw=1.5)
    a.path(f"M{n(rx - 8)} {n(ry - 50)}h16v-6h-16Z", fill=PAL["slate2"], stroke=ink, sw=.7)
    # steam generator on the floor; the U-tube carries the hot leg up and back down to the cold leg
    gx, gy = p(sx, D, 0)
    a.path(f"M{n(gx - 7)} {n(gy - 4)}V{n(gy - 60)}a7 7 0 0 1 14 0V{n(gy - 4)}a7 4 0 0 1-14 0Z",
           fill=PAL["rock1"], stroke=ink, sw=.9)
    a.path(f"M{n(gx - 7)} {n(gy - 40)}H{n(gx - 3)}V{n(gy - 54)}a3 3 0 0 1 6 0V{n(gy - 16)}H{n(gx - 7)}",
           stroke=hot, sw=1.6, fill="none", lj="round")
    a.end()
    flow(vx + 6, vx + 21, 40)
    flow(sx - 5, sx - 19.5, 16)
    a.path(outer + inner, fill=PAL["rock"], fr="evenodd")
    a.path(outer, cls="o")

    # ---- turbine hall, its front wall cut away on the same face
    def exit_x(z):
        rr = R if z <= Hc else R * math.sqrt(max(0.0, 1 - ((z - Hc) / Hd) ** 2))
        return cx + math.sqrt(rr * rr - D * D)
    hx0, hx1, HD, HH, w = 104, 188, 30, 66, 3
    tx = 128                                 # steam inlet above the turbine's high-pressure end
    fo_, to_, ro_ = box_faces(hx0, hx1, 0, HH, HD, o)
    a.path(P(to_, True), fill=PAL["rock1"])
    a.path(P(ro_, True), fill=PAL["rock"])
    op_ = [p(hx0 + w, 0, 0), p(hx0 + w, 0, HH - w), p(hx1 - w, 0, HH - w), p(hx1 - w, 0, 0)]
    a.path(P(op_, True), fill=PAL["paper2"])
    a.group(cp=a.clip(P(op_, True)))
    a.path(P([p(hx0, 0, 0), p(hx0, HD, 0), p(hx1, HD, 0), p(hx1, 0, 0)], True), fill=PAL["rock1"])
    # cooling-water pipes leave the back of the condenser at depth CD, behind the generator plinth
    CD = 20
    a.path(P([p(150, CD, 12), p(hx1, CD, 12)]) + P([p(150, CD, 5), p(hx1, CD, 5)]), stroke=water, sw=2.2,
           fill="none")
    a.end()
    # feedwater from the condenser back to the steam generator (it passes behind the hall's front frame)
    a.path(P([p(exit_x(22), D, 22), p(124, D, 22)]), stroke=water, sw=2.4, fill="none")
    flow(hx0 - 4, hx0 - 19, 22)
    a.group(cp=a.clip(P(op_, True)))
    # condenser on the floor, with the turbine resting on it and its exhaust opening into it
    draw_box(a, 120, 154, 0, 22, 16, p(0, D - 5, 0), PAL["water1"], "#eef3f4", PAL["water2"])
    a.path(P([p(tx, D, HH), p(tx, D, 40)]), stroke=steam, sw=3, fill="none")
    a.path(P([p(124, D, 32), p(150, D, 22), p(150, D, 50), p(124, D, 40)], True), fill=PAL["slate1"],
           stroke=ink, sw=.9)
    # shaft to the generator, which sits on a concrete plinth
    draw_box(a, 160, 172, 0, 25, 8, p(0, D - 4, 0), "#d8d2c6", "#e6e1d6", PAL["rock"])
    a.path(P([p(150, D, 36), p(157, D, 36)]), stroke=ink, sw=2.6)
    draw_box(a, 157, 175, 25, 47, 8, p(0, D - 4, 0), PAL["slate1"], "#c4cad0", PAL["slate"])
    a.end()
    a.path(P(fo_, True) + P(op_, True), fill=PAL["rock"], fr="evenodd")
    a.path(P(fo_, True) + P(to_, True) + P([ro_[1], ro_[2], ro_[3]]), cls="o")
    # steam line from the containment over to the roof and down into the turbine
    a.path(P([p(exit_x(84), D, 84), p(tx, D, 84), p(tx, D, HH)]), stroke=steam, sw=3, fill="none", lj="round")
    flow(hx0 - 22, hx0 - 6, 84)

    # ---- hyperboloid cooling tower standing on the block, centre at depth 26
    tcx, tcy, TH = 244, 26, 100

    def rad(z):
        return 13 * math.sqrt(1 + ((z - 72) / 52.7) ** 2)
    k = math.hypot(1, DX)
    sx0, sy0 = p(tcx, tcy, 0)
    zs = [0, 12, 28, 46, 62, 76, 88, TH]
    left = [(sx0 - k * rad(z), sy0 - z + .2 * rad(z)) for z in zs]
    right = [(sx0 + k * rad(z), sy0 - z - .2 * rad(z)) for z in zs]
    tang = math.degrees(math.atan(DX))
    base = _nuclear_ring(o, tcx, tcy, 0, rad(0), 180 + tang, 360 + tang, 24)
    rim_back = _nuclear_ring(o, tcx, tcy, TH, rad(TH), tang, 180 + tang, 24)
    # walls are splined on their own, so the rim and base stay exact ellipse arcs and the
    # silhouette meets them without the overshoot a spline through the corners would add
    outline = _nuclear_spline(left[::-1])[:-1] + base + _nuclear_spline(right)[1:-1] + rim_back
    tw = P(outline, True)
    a.path(tw, fill=a.lin([(0, "#f7f6f2"), (.55, PAL["rock1"]), (1, PAL["rock"])], 0, 0, 1, 0))
    a.path(tw, cls="o")
    # the rim is exactly the shell's top: its back half is already the outline, so only the
    # opening's fill and the front lip (at outline weight, meeting both silhouette ends) are added
    rim = _nuclear_ring(o, tcx, tcy, TH, rad(TH), 0, 360, 36)
    a.path(P(rim, True), fill=PAL["rockd"])
    a.path(P(_nuclear_ring(o, tcx, tcy, TH, rad(TH), 180 + tang, 360 + tang, 24)), cls="o", fill="none")

    # cooling water out of the condenser to the tower and back, entering the shell at depth CD
    def tower_x(z):
        return tcx - math.sqrt(rad(z) ** 2 - (tcy - CD) ** 2)
    a.path(P([p(hx1, CD, 12), p(tower_x(12), CD, 12)]) + P([p(hx1, CD, 5), p(tower_x(5), CD, 5)]),
           stroke=water, sw=2.2, fill="none")
    flow(hx1 + 10, hx1 + 22, 12, CD, 7.5)
    flow(hx1 + 22, hx1 + 10, 5, CD, 7.5)
    # the plume of condensed water vapour rising from the rim
    cxm, top = p(tcx, tcy, TH)
    plume = scallop([(cxm - 14, top - 9), (cxm - 20, top - 24), (cxm - 10, top - 37), (cxm + 6, top - 44),
                     (cxm + 22, top - 41), (cxm + 32, top - 31), (cxm + 32, top - 17), (cxm + 18, top - 9)],
                    bulge=.35, seed=5)
    a.path(plume, fill=a.lin([(0, "#fff"), (1, PAL["cloud2"])], 0, 0, .5, 1), fo=.94)
    a.path(plume, cls="h", so=.5)
    return a.render()


def solar():
    """A solar farm: rows of photovoltaic arrays on short legs, tilted toward the low Sun so its
    parallel rays strike the panels near-perpendicular; each row casts its shadow behind it, which
    sets the row spacing. The farm's footprint is traced as a polygon on the ground, the way the
    dataset outlines each installation on satellite images."""
    a = Art("amber", seed=55)
    o = (14, 166)
    Wd, D = 172.0, 142.0
    a.shadow(o[0] + 140, o[1] + 3, 140, 6, op=.18)
    _slab(a, o, Wd, D, 6, mix(PAL["sage1"], PAL["sand1"], .4))
    # direction toward the Sun (front left, high); shadows fall back-right, rays travel along -s
    sx, sy, sz = -.45, -.75, 1.2
    shp = lambda x, y, z: ob(x - z * sx / sz, y - z * sy / sz, 0, o)
    # footprint polygon on the ground, under the arrays
    poly = [(12, 9), (160, 9), (160, 134), (12, 134)]
    a.path(P([ob(x, y, 0, o) for x, y in poly], True), fill=a.AP, fo=.4)
    a.path(P([ob(x, y, 0, o) for x, y in poly], True), fill="none", stroke=a.AD, sw=1.0, da="3.2 2")
    for (x, y) in poly:
        p = ob(x, y, 0, o)
        a.path(f"M{n(p[0] - 1.8)} {n(p[1] - 1.8)}h3.6v3.6h-3.6Z", fill="#fff", stroke=a.AD, sw=.7)
    # PV rows: low edge at y (front, facing the Sun), high edge tilt_d further back
    x0, x1 = 22.0, 150.0
    z_lo, z_hi, tilt_d = 3.5, 11.0, 10.0
    legs = [x0 + 6 + k * (x1 - x0 - 12) / 3 for k in range(4)]
    rows = [20.0 + j * 31.0 for j in range(4)]
    hits = []
    for y in sorted(rows, reverse=True):
        yb = y + tilt_d
        a.path(P([shp(x0, y, z_lo), shp(x1, y, z_lo), shp(x1, yb, z_hi), shp(x0, yb, z_hi)], True), fill=PAL["ink"], fo=.16)
        for lx in legs:  # rear legs, mostly hidden behind the panel
            a.path(P([ob(lx, yb, 0, o), ob(lx, yb, z_hi, o)]), stroke=PAL["rockd"], sw=1.4)
        lo = [ob(x0, y, z_lo, o), ob(x1, y, z_lo, o)]
        hi = [ob(x1, yb, z_hi, o), ob(x0, yb, z_hi, o)]
        a.path(P(lo + hi, True), fill=a.lin([(0, PAL["slate2"]), (1, PAL["slated"])], 0, 1, 0, 0), stroke=PAL["ink"], sw=.55)
        grid = "".join(P([lerp2(lo[0], lo[1], k / 6), lerp2(hi[1], hi[0], k / 6)]) for k in range(1, 6))
        grid += P([lerp2(lo[0], hi[1], .5), lerp2(lo[1], hi[0], .5)])
        a.path(grid, stroke="#fff", sw=.5, so=.4, fill="none")
        a.path(P([lerp2(hi[1], lo[0], .12), lerp2(hi[0], lo[1], .12)]), stroke="#fff", sw=.7, so=.35)
        for lx in legs:  # front legs, in front of the ground and under the low edge
            a.path(P([ob(lx, y, 0, o), ob(lx, y, z_lo, o)]), stroke=PAL["rockd"], sw=1.4)
        hits.append((y, lo, hi))
    # the Sun and parallel rays ending on the panel faces
    # screen direction of travel of the light, -s projected with the block's own oblique map
    q0, q1 = ob(0, 0, 0), ob(-sx, -sy, -sz)
    dx, dy = q1[0] - q0[0], q1[1] - q0[1]
    d0 = math.hypot(dx, dy)
    dx, dy = dx / d0, dy / d0
    # rays land on the back row: on screen nothing lies above it, and in 3D the row in front
    # does not block them (its shadow stops short of the next row)
    y, lo, hi = hits[0]
    ends = [lerp2(lerp2(lo[0], hi[1], .55), lerp2(lo[1], hi[0], .55), t) for t in (.15, .45, .75)]
    cx = sum(e[0] for e in ends) / len(ends); cy = sum(e[1] for e in ends) / len(ends)
    sun = (cx - dx * 95, cy - dy * 95)
    a.circle(sun, 16, fill=a.rad([(0, a.AP, 1), (1, a.AP, 0)]))
    a.circle(sun, 10, fill=a.rad([(0, "#fff7e2"), (1, a.AL)]), stroke=a.A, sw=.7)
    for e in ends:
        t = (e[0] - sun[0]) * dx + (e[1] - sun[1]) * dy
        st = (e[0] - dx * (t - 20), e[1] - dy * (t - 20))
        a.arrow([st, e], color=a.A, w=.9, head=4.2, curve=False)
    return a.render()


def _mines_halfring(pc, rx, ry, z, o, k=40):
    """The back half (y >= 0) of a horizontal ellipse centred on the front edge of the block."""
    return [ob(pc[0] + rx * math.cos(t), pc[1] + ry * math.sin(t), z, o)
            for t in [math.pi * i / k for i in range(k + 1)]]


def _mines_hull(pts):
    """Convex hull (monotone chain) of screen points, for the sides of a terraced dump."""
    pts = sorted(set(pts))

    def cross(o_, a_, b_):
        return (a_[0] - o_[0]) * (b_[1] - o_[1]) - (a_[1] - o_[1]) * (b_[0] - o_[0])
    lo, hi = [], []
    for p in pts:
        while len(lo) >= 2 and cross(lo[-2], lo[-1], p) <= 0:
            lo.pop()
        lo.append(p)
    for p in reversed(pts):
        while len(hi) >= 2 and cross(hi[-2], hi[-1], p) <= 0:
            hi.pop()
        hi.append(p)
    return lo[:-1] + hi[:-1]


def mines():
    """An open-pit mine cut in half by the front face of the block: stepped benches with the
    haul road winding down them to the ore body, which shows in section below the pit floor,
    a terraced waste dump of stripped overburden beside the pit, and the disturbed area traced
    as a polygon as in the dataset."""
    a = Art("amber", seed=57)
    o = (10, 134)
    Wd, D, base = 190.0, 168.0, 40.0
    pc = (84.0, 0.0)                       # the pit is centred on the front edge
    z0 = 6.0                               # ground level (flat)
    N, bh = 4, 6.0                         # benches and bench height
    zs = [z0 - k * bh for k in range(N + 1)]
    rs = [(72.0 - k * 12.5, 66.0 - k * 11.5) for k in range(N + 1)]
    toe = 2.5                              # how far each wall leans in from rim to foot

    a.shadow(o[0] + 160, o[1] + base + 2, 135, 5, op=.2)
    terrain(a, o, Wd, D, lambda x, y: z0, nx=4, ny=4, base=base,
            colours={"land": mix(PAL["sage2"], PAL["sand1"], .3)},
            strata=[(3, PAL["umber"]), (10, PAL["sand2"]), (11, PAL["clay"]), (99, PAL["rock"])])

    def notch(k):
        """The stepped cut on the front face from level k down to the pit floor."""
        left, right = [], []
        for j in range(k, N):
            left += [(pc[0] - rs[j][0], zs[j]), (pc[0] - rs[j][0] + toe, zs[j + 1])]
            right += [(pc[0] + rs[j][0], zs[j]), (pc[0] + rs[j][0] - toe, zs[j + 1])]
        return [ob(x, 0, z, o) for x, z in left + right[::-1]]

    # haul road: one ramp graded down the far side of the pit (where it can be seen), from the
    # ground beyond the rim on the left to the pit floor; it keeps to the cone through the bench
    # edges, so it lies on the stepped walls rather than above or inside them
    road = []
    for i in range(49):
        t = i / 48
        ang = math.radians(lerp(150, 62, t))
        z = lerp(zs[0], zs[N], t)
        rx, ry = lerp(rs[0][0], rs[N][0] - 6, t), lerp(rs[0][1], rs[N][1] - 6, t)
        road.append(ob(pc[0] + rx * math.cos(ang), pc[1] + ry * math.sin(ang), z + .3, o))
    r_in = math.radians(157)                    # where it comes in over the ground
    road.insert(0, ob(pc[0] + (rs[0][0] + 12) * math.cos(r_in), pc[1] + (rs[0][1] + 12) * math.sin(r_in), z0 + .3, o))

    # the benches, painted from the rim inward; each level is clipped to its own opening
    for k in range(N + 1):
        rx, ry = rs[k]
        rim = _mines_halfring(pc, rx, ry, zs[k], o)
        if k == N:                              # pit floor: the top of the ore body, exposed
            a.path(P(rim, True), fill=a.AL)
            break
        a.group(cp=a.clip(P(rim, True) + P(notch(k), True)))
        t = k / (N - 1)
        a.path(P(rim, True) + P(notch(k), True), fill=mix(PAL["clay"], PAL["umber"], .1 + .5 * t))   # wall
        foot = _mines_halfring(pc, rx - toe, ry - toe, zs[k + 1], o)
        a.path(P(foot, True), fill=mix(PAL["sand2"], PAL["soil"], .2 + .8 * t))                      # bench
        a.path(P(foot), cls="h", so=.45)
        a.end()
    a.path(smooth(road[:1] + road[1::4]), stroke=PAL["white"], sw=1.4, fill="none", lc="round", so=.9)

    # ore body in section on the front face; its top is the pit floor
    fx, zf = rs[N][0], zs[N]
    ore = [ob(pc[0] + dx, 0, z, o) for dx, z in
           ((-fx, zf), (fx, zf), (fx + 24, zf - 2.5), (fx + 34, zf - 6.5), (fx + 16, zf - 10),
            (-8, zf - 10.5), (-fx - 26, zf - 8.5), (-fx - 32, zf - 4.5), (-fx - 16, zf - 2))]
    a.path(smooth(ore, True), fill=a.lin([(0, a.AL), (1, a.A)], 0, 0, 0, 1))
    a.path(smooth(ore, True), cls="h", so=.8)
    a.path(P(notch(0)), cls="o")              # the cut through the pit

    # disturbed-area footprint (dataset polygon), open along the front where the block is cut;
    # drawn before the dump, which stands inside it and hides the line behind it
    poly = [(6, 0), (6, 72), (28, 88), (118, 90), (126, 128), (184, 130), (184, 62), (164, 42), (164, 0)]
    a.path(P([ob(x, y, z0 + .4, o) for x, y in poly]), fill="none", stroke=a.AD, sw=1.0, da="3 1.8")

    # terraced waste dump of stripped overburden, beside the pit
    wd = (154, 100)
    for k in range(3):
        rx, ry, zl = 26 - k * 7.5, 22 - k * 6, z0 + k * 5

        def ring(r1, r2, z):
            return [ob(wd[0] + r1 * math.cos(t), wd[1] + r2 * math.sin(t), z, o) for t in [2 * math.pi * i / 40 for i in range(40)]]
        top = ring(rx - 3, ry - 3, zl + 5)
        a.path(P(_mines_hull(ring(rx, ry, zl) + top), True), fill=mix(PAL["rock"], PAL["ink"], .1))   # slope
        a.path(P(top, True), fill=PAL["rock1"])                                                   # terrace
        a.path(P(top, True), cls="h", so=.45)

    return a.render()
