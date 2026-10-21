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


def wind_turbines():
    """A modern turbine with its nacelle cut open (main shaft, gearbox, generator), the swept rotor
    disc facing a wind that strengthens with height (the logarithmic profile), and a wind farm
    behind on the plains with every rotor turned to the same wind."""
    a = Art("cobalt", seed=51)
    o = (8, 174)
    Wd, D = 292.0, 92.0

    def fields():
        for j in range(4):
            for i in range(8):
                q = [ob(Wd * i / 8, D * j / 4, 0, o), ob(Wd * i / 8, D * (j + 1) / 4, 0, o),
                     ob(Wd * (i + 1) / 8, D * (j + 1) / 4, 0, o), ob(Wd * (i + 1) / 8, D * j / 4, 0, o)]
                a.path(P(q, True), fill=(PAL["sage1"], PAL["sage2"], PAL["sand1"], mix(PAL["sage2"], PAL["sand2"], .5))[(i * 3 + j * 5) % 4])
        a.path("".join(P([ob(0, D * j / 4, 0, o), ob(Wd, D * j / 4, 0, o)]) for j in range(1, 4)) +
               "".join(P([ob(Wd * i / 8, 0, 0, o), ob(Wd * i / 8, D, 0, o)]) for i in range(1, 8)), stroke="#fff", sw=.6, so=.7)
    a.shadow(o[0] + 180, o[1] + 2, 170, 5, op=.18)
    _slab(a, o, Wd, D, 5, PAL["sage1"], fields=fields)
    # the far turbines, back to front, smaller with distance
    for (x, y) in ((150, 86), (196, 84), (242, 82), (176, 60), (222, 58), (268, 56), (204, 34), (250, 32)):
        b = ob(x, y, 0, o)
        s = 1 - y / 140
        a.shadow(b[0] + 3, b[1] + .5, 4 * s, 1, op=.25)
        _turbine(a, b, 52 * s, 34 * s, ang=(x * 7) % 120, detail=False, colour="#f4f4f1")
    # access track
    a.path(smooth([ob(40, 0, 0, o), ob(70, 30, 0, o), ob(150, 60, 0, o), ob(250, 84, 0, o)]), stroke="#fff", sw=1.2, fill="none", so=.85)
    # ---- the near turbine
    base = ob(60, 22, 0, o)
    a.shadow(base[0] + 8, base[1] + 1, 12, 2, op=.3)
    R = 62
    # swept area (dashed ellipse) first, behind the blades
    hub_pt = (base[0], base[1] - 98)
    a.ellipse(hub_pt, R * .5, R, fill=a.AP, fo=.25, stroke=a.A, sw=.6, da="2 1.6")
    hub, nac = _turbine(a, base, 98, R, ang=-28, tilt=.5)
    # nacelle cutaway window: main shaft, main bearing, gearbox, generator
    nx0, ny0 = hub[0] + 3, hub[1] - 4
    a.path(f"M{n(nx0)} {n(ny0)}h{n(R * .32 - 5)}v8h{n(-(R * .32 - 5))}Z", fill=PAL["paper2"], stroke=PAL["ink"], sw=.45)
    a.path(f"M{n(nx0)} {n(hub[1])}h6", stroke=PAL["ink"], sw=1.1)
    a.path(f"M{n(nx0 + 2)} {n(hub[1] - 2)}h2.4v4h-2.4Z", fill=PAL["slate1"], stroke=PAL["ink"], sw=.35)
    a.path(f"M{n(nx0 + 6)} {n(hub[1] - 3)}h4.6v6h-4.6Z", fill=PAL["slate"], stroke=PAL["ink"], sw=.4)
    a.path(f"M{n(nx0 + 10.6)} {n(hub[1])}h1.6", stroke=PAL["ink"], sw=.7)
    a.path(f"M{n(nx0 + 12.2)} {n(hub[1] - 2.6)}h3.6v5.2h-3.6Z", fill=a.A, stroke=PAL["ink"], sw=.4)
    # ---- wind profile: speed grows with height above the ground (log law)
    gx, gy = 4, base[1] - 2
    for k, zz in enumerate((8, 26, 50, 78, 108, 140)):
        y = gy - zz * .95
        L = 12 + 9 * math.log(1 + zz / 6)
        a.arrow([(gx + 6, y), (gx + 6 + L, y)], color=a.A, w=.9, head=3.8, curve=False)
    prof = [(gx + 6 + 12 + 9 * math.log(1 + zz / 6) - 3, gy - zz * .95) for zz in range(4, 141, 8)]
    a.path(smooth(prof), cls="h", so=.6)
    a.path(P([(gx + 6, gy), (gx + 6, gy - 136)]), cls="h", so=.6)
    return a.render()


def nuclear():
    """A pressurised-water reactor in section: the containment building with the reactor vessel,
    core and control rods, the primary loop (hot leg, steam generator, pump, cold leg) and
    pressuriser; steam driving the turbine and generator; the condenser and the cooling tower
    venting water vapour."""
    a = Art("verm", seed=53)
    gy = 160
    o = (6, gy + 6)
    a.shadow(166, gy + 10, 160, 6, op=.2)
    f, t, r = box_faces(0, 300, -6, 0, 34, (6, gy + 6))
    a.path(P(t, True), fill=PAL["sage1"])
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    gy2 = gy - 4
    # ---- containment: cylinder with a dome, cut open
    cx0, cx1 = 18, 112
    top = 70
    shell = (f"M{cx0} {gy2}V{top}A{(cx1 - cx0) / 2} 34 0 0 1 {cx1} {top}V{gy2}Z")
    a.path(shell, fill=PAL["rock1"])
    inner = (f"M{cx0 + 5} {gy2}V{top + 2}A{(cx1 - cx0) / 2 - 5} 29 0 0 1 {cx1 - 5} {top + 2}V{gy2}Z")
    a.path(inner, fill=a.lin([(0, "#f6f4ef"), (1, PAL["paper2"])], 0, 0, 0, 1))
    a.path(shell, fill=a.hatch(PAL["ink"], 45, 2.2, .35, .35))
    a.path(inner, fill=a.lin([(0, "#f6f4ef"), (1, PAL["paper2"])], 0, 0, 0, 1))
    a.path(shell, cls="o")
    a.path(inner, cls="h", so=.6)
    # reactor pressure vessel with core and control rods
    vx, vy = 44, 104
    a.path(f"M{vx - 11} {vy}v34a11 7 0 0 0 22 0v-34a11 6 0 0 0-22 0Z", fill=PAL["slate1"], stroke=PAL["ink"], sw=.8)
    a.path(f"M{vx - 7} {vy + 14}h14v16h-14Z", fill=a.rad([(0, a.AP), (1, a.AL)]))
    a.path("".join(f"M{vx - 6 + k * 2.4} {vy + 15}v14" for k in range(6)), stroke=a.AD, sw=.8, so=.85)
    a.path("".join(f"M{vx - 4.8 + k * 4.8} {vy - 10}v{20 + (k % 2) * 3}" for k in range(3)), stroke=PAL["ink"], sw=.9)
    a.path(f"M{vx - 8} {vy - 12}h16v4h-16Z", fill=PAL["slate2"])
    # steam generator (tall vessel) and pressuriser
    sx = 86
    a.path(f"M{sx - 8} 140V92a8 8 0 0 1 16 0V140a8 5 0 0 1-16 0Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.8)
    a.path("".join(f"M{sx - 4 + k * 2.6} 136V104a1.3 1.3 0 0 1 2.6 0" for k in range(0, 3)), stroke=a.A, sw=.6, fill="none")
    a.path(f"M64 98v-10a4 4 0 0 1 8 0v10a4 3 0 0 1-8 0Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.7)
    # primary loop: hot leg to the generator tubes, cold leg back through the pump
    a.path(f"M{vx + 11} 112H{sx - 8}", stroke=a.A, sw=2.4, fill="none")
    a.path(f"M{sx - 8} 136H{vx + 22}V132H{vx + 11}", stroke=a.AD, sw=2.4, fill="none", lj="round")
    a.circle((vx + 22, 136), 3.6, fill=PAL["slate2"], stroke=PAL["ink"], sw=.6)
    a.path(f"M68 98V112", stroke=a.A, sw=1.2, fill="none")
    a.arrow([(52, 109.5), (64, 109.5)], color="#fff", w=.6, head=2.8, curve=False)
    a.arrow([(76, 139.5), (62, 139.5)], color="#fff", w=.6, head=2.8, curve=False)
    # ---- secondary loop: steam to the turbine, condensate back
    tb0, tb1 = 124, 210
    a.path(f"M{tb0} {gy2}V96H{tb1}V{gy2}Z", fill=PAL["paper2"], stroke=PAL["ink"], sw=.9)
    a.path(f"M{tb0} 96l6-8H{tb1 - 6}l6 8", fill=PAL["rock1"], stroke=PAL["ink"], sw=.7)
    steam = PAL["water4"]
    a.path(f"M{sx} 84V78H138V112", stroke=steam, sw=2.0, fill="none", lj="round")
    a.arrow([(104, 76), (122, 76)], color=steam, w=.8, head=3.4, curve=False)
    # turbine (expanding stages) and generator on one shaft
    a.path("M138 106l26-6v24l-26-6Z", fill=PAL["slate1"], stroke=PAL["ink"], sw=.7)
    a.path("".join(f"M{142 + k * 5} {106 - k * 1.2}v{8 + k * 2.4}" for k in range(5)), cls="h", so=.6)
    a.path("M164 112h6", stroke=PAL["ink"], sw=1.6)
    a.path("M170 103h22v18h-22Z", fill=a.lin([(0, PAL["rock1"]), (1, PAL["slate"])], 0, 0, 0, 1), stroke=PAL["ink"], sw=.7)
    a.path("M174 103v18M178 103v18M182 103v18M186 103v18", cls="h", so=.5)
    # condenser below the turbine with cooling-water tubes
    a.path("M140 132h40v14h-40Z", fill=PAL["water1"], stroke=PAL["ink"], sw=.7)
    a.path("M144 136h32M144 139h32M144 142h32", stroke=steam, sw=.5)
    a.path(f"M140 140H{sx + 8}", stroke=steam, sw=1.6, fill="none")
    # cooling water to and from the tower
    a.path("M180 136H236V150M180 142H230V150", stroke=steam, sw=1.2, fill="none", lj="round")
    # transmission: pylon and lines from the generator
    # ---- hyperboloid cooling tower with its plume of condensed vapour
    cxm, base_w, top_w, waist = 266, 34, 22, 18
    tw = (f"M{cxm - base_w} {gy2}C{cxm - waist} 118 {cxm - waist} 96 {cxm - top_w} 56H{cxm + top_w}"
          f"C{cxm + waist} 96 {cxm + waist} 118 {cxm + base_w} {gy2}Z")
    a.path(tw, fill=a.lin([(0, "#f7f6f2"), (.6, PAL["rock1"]), (1, PAL["rock"])], 0, 0, 1, 0))
    a.path("".join(f"M{cxm - base_w + k * 4} {gy2}l2-5" for k in range(18)), stroke=PAL["ink"], sw=.4, so=.6)
    a.path(tw, cls="o")
    a.ellipse((cxm, 56), top_w, 3, fill=PAL["rock"], stroke=PAL["ink"], sw=.7)
    plume = scallop([(cxm - 22, 54), (cxm - 26, 34), (cxm - 14, 18), (cxm + 4, 10), (cxm + 24, 12), (cxm + 40, 24),
                     (cxm + 42, 40), (cxm + 22, 52)], bulge=.35, seed=5)
    a.path(plume, fill=a.lin([(0, "#fff"), (1, PAL["cloud2"])], 0, 0, .5, 1), fo=.92)
    a.path(plume, cls="h", so=.45)
    a.arrow([(cxm, 150), (cxm, 70)], color=PAL["ink"], w=.7, head=3.6, curve=False, op=.6)
    return a.render()


def solar():
    """A solar farm: rows of photovoltaic arrays tilted toward the Sun, the inverter station and
    access tracks, and the installation's footprint traced as a polygon, the way the dataset
    outlines each farm on satellite images."""
    a = Art("amber", seed=55)
    o = (8, 166)
    Wd, D = 184.0, 170.0
    a.shadow(o[0] + 150, o[1] + 3, 150, 6, op=.18)
    _slab(a, o, Wd, D, 6, mix(PAL["sage1"], PAL["sand1"], .4))
    rnd = a.rng
    a.dots([ob(rnd.uniform(2, Wd - 2), rnd.uniform(2, D - 2), 0, o) for _ in range(260)], r=.35, color=PAL["moss"], op=.5)
    # footprint polygon (drawn first, under the arrays)
    poly = [(14, 12), (170, 12), (170, 96), (138, 96), (138, 156), (14, 156)]
    a.path(P([ob(x, y, 0, o) for x, y in poly], True), fill=a.AP, fo=.35)
    # tracks
    a.path(P([ob(6, 84, 0, o), ob(Wd - 4, 84, 0, o)]) + P([ob(80, 4, 0, o), ob(80, D - 4, 0, o)]), stroke=PAL["sand2"], sw=2.2, fill="none")
    # PV rows: each row is a tilted panel strip facing the Sun (to the front, i.e. south)
    tilt_h, tilt_d = 5.0, 6.0
    rows = []
    for j in range(13):
        y = 146 - j * 10.6
        if 76 < y < 92:
            continue
        rows.append((18, 74, y))
        rows.append((86, 132 if y > 90 else 166, y))
    for (x0, x1, y) in sorted(rows, key=lambda r: -r[2]):
        lo = [ob(x0, y, 1.2, o), ob(x1, y, 1.2, o)]
        hi = [ob(x1, y + tilt_d, 1.2 + tilt_h, o), ob(x0, y + tilt_d, 1.2 + tilt_h, o)]
        a.path(P([ob(x0, y + tilt_d, 0, o), ob(x1, y + tilt_d, 0, o), ob(x1 + 2, y + tilt_d + 3, 0, o), ob(x0 + 2, y + tilt_d + 3, 0, o)], True),
               fill=PAL["ink"], fo=.12)
        a.path(P(lo + hi, True), fill=a.lin([(0, PAL["slate2"]), (1, PAL["slated"])], 0, 1, 0, 0), stroke=PAL["ink"], sw=.4)
        cells = "".join(P([lerp2(lo[0], lo[1], k / 12), lerp2(hi[1], hi[0], k / 12)]) for k in range(1, 12))
        cells += P([lerp2(lo[0], hi[1], .5), lerp2(lo[1], hi[0], .5)])
        a.path(cells, stroke="#fff", sw=.3, so=.45, fill="none")
        a.path(P([lerp2(lo[0], hi[1], .15), lerp2(lo[1], hi[0], .15)]), stroke="#fff", sw=.4, so=.35)
    # inverter station and fence posts
    iv = ob(146, 120, 0, o)
    f_, t_, r_ = box_faces(0, 12, 0, 7, 8, iv)
    a.path(P(f_, True), fill=PAL["paper2"])
    a.path(P(t_, True), fill="#fff")
    a.path(P(r_, True), fill=PAL["rock1"])
    a.path(P(f_, True) + P(t_[1:3]) + P([r_[1], r_[2], r_[3]]), cls="h", so=.8)
    a.path(P([ob(x, y, 0, o) for x, y in poly], True), fill="none", stroke=a.AD, sw=1.0, da="3 1.8")
    for (x, y) in poly:
        p = ob(x, y, 0, o)
        a.path(f"M{n(p[0] - 1.6)} {n(p[1] - 1.6)}h3.2v3.2h-3.2Z", fill="#fff", stroke=a.AD, sw=.7)
    # the Sun and parallel rays striking the panels
    sun = (34, 26)
    a.circle(sun, 15, fill=a.rad([(0, a.AP, 1), (1, a.AP, 0)]))
    a.circle(sun, 9, fill=a.rad([(0, "#fff7e2"), (1, a.AL)]), stroke=a.A, sw=.6)
    for k in range(4):
        p0 = (sun[0] + 16 + k * 20, sun[1] + 6 + k * 2)
        a.arrow([p0, (p0[0] + 40, p0[1] + 40)], color=a.A, w=.8, head=3.8, curve=False)
    return a.render()


def mines():
    """An open-pit mine in a cut-away block: stepped benches down to the ore body in section,
    the spiral haul road, a terraced waste dump and a tailings pond behind its dam, with the
    whole disturbed area traced as a polygon as in the dataset."""
    a = Art("amber", seed=57)
    o = (10, 152)
    Wd, D = 200.0, 150.0

    def h(x, y):
        return 6 + 3 * math.sin(x / 31) * math.cos(y / 27)
    a.shadow(o[0] + 160, o[1] + 4, 160, 7, op=.2)
    terrain(a, o, Wd, D, h, nx=20, ny=8, colours={"land": mix(PAL["sage2"], PAL["sand1"], .3)}, base=22,
            strata=[(3, PAL["umber"]), (6, PAL["sand2"]), (8, PAL["clay"]), (40, PAL["rock"])])
    # ore body in section on the front face, under the pit
    pc = (80.0, 64.0)
    ore = [ob(pc[0] + 26 + dx, 0, z, o) for dx, z in ((-44, -6), (-20, -3), (16, -4), (40, -9), (30, -16), (-6, -19), (-38, -15))]
    a.path(smooth(ore, True), fill=a.lin([(0, a.AL), (1, a.A)], 0, 0, 0, 1))
    a.path(smooth(ore, True), fill=a.hatch(a.AD, -30, 2.2, .4, .5))
    a.path(smooth(ore, True), cls="h", so=.8)
    # the pit: concentric benches on the surface plane, each one lower; back walls show
    benches, bh = 7, 4.2

    def ell(rx, ry, z):
        return P([ob(pc[0] + rx * math.cos(t), pc[1] + ry * math.sin(t), z, o) for t in [2 * math.pi * i / 48 for i in range(48)]], True)
    radii = [(62 - k * 8, 52 - k * 6.8) for k in range(benches + 1)]
    for k in range(benches):
        z = h(*pc) - k * bh
        a.path(ell(*radii[k], z), fill=mix(PAL["sand2"], PAL["clay"], k / benches))          # bench floor
        a.path(ell(*radii[k], z), cls="h", so=.4)
        a.path(ell(*radii[k + 1], z), fill=mix(PAL["clay"], PAL["umber"], .2 + .6 * k / benches))  # wall (back shows)
    bot = ob(pc[0], pc[1], h(*pc) - benches * bh, o)
    a.ellipse(bot, 7, 2.5, fill=PAL["water3"], stroke=PAL["ink"], sw=.4)
    # spiral haul road
    road = []
    for i in range(70):
        t = i / 69
        ang = math.radians(200 + 600 * t)
        rx, ry = lerp(62, 10, t), lerp(52, 8, t)
        road.append(ob(pc[0] + rx * math.cos(ang), pc[1] + ry * math.sin(ang), h(*pc) - t * benches * bh + 1, o))
    a.path(smooth(road[::2]), stroke=PAL["paper"], sw=1.1, fill="none", so=.9)
    for k in (8, 30, 52):
        p = road[k]
        a.path(f"M{n(p[0] - 1.6)} {n(p[1] - 1.2)}h3.2v2h-3.2Z", fill=a.A, stroke=PAL["ink"], sw=.3)
    # terraced waste dump (front right) and tailings pond with its dam (back right)
    wd = (168, 34)
    for k in range(4):
        rx, ry, z = 26 - k * 6, 14 - k * 3.2, h(*wd) + k * 4.4
        sh = [ob(wd[0] + rx * math.cos(t), wd[1] + ry * math.sin(t), z, o) for t in [2 * math.pi * i / 36 for i in range(36)]]
        sh2 = [ob(wd[0] + rx * math.cos(t), wd[1] + ry * math.sin(t), z + 4.4, o) for t in [2 * math.pi * i / 36 for i in range(36)]]
        a.path(P(sh, True), fill=mix(PAL["rock"], PAL["ink"], .15))
        a.path(P(sh2, True), fill=PAL["rock1"])
        a.path(P(sh2, True), cls="h", so=.4)
    tp = [ob(x, y, h(x, y) + .5, o) for x, y in ((150, 90), (194, 90), (196, 140), (148, 140))]
    a.path(P(tp, True), fill=PAL["water2"], stroke=PAL["rock"], sw=2.4, lj="round")
    a.path(P(tp, True), cls="h", so=.6)
    # disturbed-area polygon (dataset outline)
    poly = [(10, 6), (138, 6), (146, 16), (198, 10), (198, 146), (142, 146), (124, 126), (10, 126)]
    a.path(P([ob(x, y, h(x, y) + .4, o) for x, y in poly], True), fill="none", stroke=a.AD, sw=1.0, da="3 1.8")
    for (x, y) in poly:
        p = ob(x, y, h(x, y) + .4, o)
        a.path(f"M{n(p[0] - 1.5)} {n(p[1] - 1.5)}h3v3h-3Z", fill="#fff", stroke=a.AD, sw=.7)
    return a.render()
