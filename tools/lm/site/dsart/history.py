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


def heritage():
    """A Doric temple on its acropolis, drawn in oblique with stepped stylobate, fluted columns,
    entablature and pedimented roof, inside the property's core zone and the wider buffer zone
    that World Heritage nominations map."""
    a = Art("verm", seed=81)
    o = (12, 160)
    Wd, D = 200.0, 120.0

    def h(x, y):
        return 4 + 22 * math.exp(-((x - 110) ** 2 / 3200 + (y - 64) ** 2 / 2000)) + 1.5 * math.sin(x / 13) * math.cos(y / 17)
    a.shadow(o[0] + 160, o[1] + 4, 150, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=26, ny=12, colours={"land": mix(PAL["sage2"], PAL["sand2"], .35)}, base=12,
                   strata=[(2.5, PAL["umber"]), (5, PAL["sand2"]), (99, PAL["rock1"])])
    # olive trees on the slopes
    rnd = a.rng
    trees = []
    for j in range(10, 0, -1):
        for i in range(18):
            x, y = 6 + i * 11.6 + rnd.uniform(-3, 3), j * 11.6 + rnd.uniform(-3, 3)
            if 2 < x < Wd - 2 and 2 < y < D - 2 and not (70 < x < 150 and 30 < y < 100) and rnd.random() < .5:
                trees.append((x, y))
    for (x, y) in trees:
        p = surf(x, y)
        a.path(broadleaf(p[0], p[1], 6, seed=int(x * y)), fill=PAL["moss"], stroke=PAL["forest"], sw=.35)
    # buffer zone and core zone
    a.path(P(_ellipse_on(surf, 110, 62, 92, 50), True), fill="none", stroke=a.AL, sw=1.0, da="4 2.4")
    a.path(P(_ellipse_on(surf, 110, 62, 52, 30), True), fill=a.AP, fo=.4, stroke=a.A, sw=1.1, da="3 1.8")
    # ---- the temple (hexastyle peripteral), front on the near side
    zb = h(110, 64) - .5
    tx0, tx1, ty0, ty1 = 86.0, 136.0, 40.0, 100.0
    stone, stone_d = "#f4efe4", PAL["rock1"]
    for k in range(3):           # crepidoma: three steps
        box3(a, tx0 - 6 + 2 * k, tx1 + 6 - 2 * k, ty0 - 6 + 2 * k, ty1 + 6 - 2 * k, zb + 2.2 * k, zb + 2.2 * (k + 1), o,
             stone, "#faf7f0", stone_d, sw=.3)
    zs = zb + 6.6
    colh = 25.0
    box3(a, tx0 + 7, tx1 - 7, ty0 + 8, ty1 - 6, zs, zs + colh, o, PAL["sand1"], None, mix(PAL["sand1"], PAL["ink"], .15), sw=.3)  # cella

    def column(x, y):
        b, t_ = ob(x, y, zs, o), ob(x, y, zs + colh, o)
        w = 3.6
        g = a.lin([(0, "#fffdf8"), (.5, stone), (1, stone_d)], b[0] - w / 2, 0, b[0] + w / 2, 0, user=True)
        a.path(f"M{n(b[0] - w / 2)} {n(b[1])}L{n(t_[0] - w * .4)} {n(t_[1])}H{n(t_[0] + w * .4)}L{n(b[0] + w / 2)} {n(b[1])}Z", fill=g,
               stroke=PAL["ink"], sw=.3)
        a.path(f"M{n(b[0])} {n(b[1])}L{n(t_[0])} {n(t_[1])}", stroke=PAL["ink"], sw=.2, so=.35)
        a.path(f"M{n(t_[0] - w * .65)} {n(t_[1])}h{n(w * 1.3)}v-1.2h{n(-w * 1.3)}Z", fill=stone, stroke=PAL["ink"], sw=.25)
    side = [ty1 - 2 - k * (ty1 - ty0 - 4) / 7 for k in range(8)]
    for y in side:                  # right flank, back to front
        column(tx1 - 2, y)
    for k in range(6):              # the front hexastyle
        column(tx0 + 2 + k * (tx1 - tx0 - 4) / 5, ty0 + 2)
    ze = zs + colh + 1.2
    box3(a, tx0, tx1, ty0, ty1, ze, ze + 4, o, stone, None, stone_d, sw=.35)           # architrave and frieze
    fr = [ob(tx0 + 1 + k * (tx1 - tx0 - 2) / 12, ty0, ze + 2.2, o) for k in range(13)]
    a.path("".join(f"M{n(p[0])} {n(p[1])}v-1.6" for p in fr), stroke=PAL["ink"], sw=.6, so=.55)     # triglyphs
    fl, fr_, rf = gable(a, tx0, tx1, ty0, ty1, ze + 4, 10, o, PAL["clay"])
    a.path(P([fl, rf, fr_], True), fill="#f6f1e6", stroke=PAL["ink"], sw=.45)                         # pediment
    a.path(P([lerp2(fl, rf, .15), lerp2(rf, fl, .0), lerp2(fr_, rf, .15)]), stroke=PAL["ink"], sw=.25, so=.5, fill="none")
    return a.render()


def ancient_places():
    """A tell cut in half: centuries of settlement layers piled into a mound, mudbrick walls and
    pottery in the section, an excavation trench on top, and the single representative point a
    gazetteer gives the place."""
    a = Art("verm", seed=83)
    o = (10, 150)
    Wd, D = 214.0, 104.0

    def h(x, y):
        return 4 + 44 * math.exp(-((x - 112) ** 2 / 2300 + (y - 20) ** 2 / 2600)) + 1.2 * math.sin(x / 9)
    layers = [(2.2, PAL["soil"]), (3, PAL["sand2"]), (2.4, PAL["clay"]), (3.4, mix(PAL["sand2"], PAL["soil"], .5)),
              (2, PAL["umber"]), (3.6, PAL["sand1"]), (2.8, PAL["clay"]), (3.2, mix(PAL["sand2"], PAL["clay"], .5)),
              (2.6, PAL["soil"]), (99, PAL["rock1"])]
    a.shadow(o[0] + 170, o[1] + 4, 160, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=30, ny=10, colours={"land": mix(PAL["sand1"], PAL["sage2"], .45)}, base=16, strata=layers)
    # mudbrick walls and sherds within the section (front face, y = 0)
    for (x, depth, w, hh) in ((88, 7, 8, 6), (128, 8, 7, 6.4), (102, 18, 9, 5.6), (140, 20, 7, 5), (112, 30, 10, 6)):
        z = h(x, 0) - depth
        p = ob(x, 0, z, o)
        a.path(f"M{n(p[0])} {n(p[1])}h{n(w)}v{n(-hh)}h{n(-w)}Z", fill=PAL["rock1"], stroke=PAL["ink"], sw=.35)
        a.path("".join(f"M{n(p[0])} {n(p[1] - hh * k / 3)}h{n(w)}" for k in (1, 2)) +
               f"M{n(p[0] + w / 2)} {n(p[1])}v{n(-hh / 3)}M{n(p[0] + w / 3)} {n(p[1] - hh / 3)}v{n(-hh / 3)}", stroke=PAL["ink"], sw=.25, so=.6)
    rnd = a.rng
    sherds = ""
    for _ in range(16):
        x = rnd.uniform(70, 160)
        z = h(x, 0) - rnd.uniform(3, 30)
        if z < 2:
            continue
        p = ob(x, 0, z, o)
        sherds += f"M{n(p[0])} {n(p[1])}q1.2-1.4 2.6 0"
    a.path(sherds, stroke=a.A, sw=1.0, fill="none", lc="round")
    # excavation trench (a grid of squares) on the summit
    tr = []
    for i in range(3):
        for j in range(2):
            x0, y0 = 98 + i * 9, 40 + j * 9
            tr.append(P([surf(x0, y0, -1.5), surf(x0, y0 + 8, -1.5), surf(x0 + 8, y0 + 8, -1.5), surf(x0 + 8, y0, -1.5)], True))
    a.path("".join(tr), fill=PAL["clay"], stroke=PAL["ink"], sw=.4)
    # representative point: a survey pin with its crosshair
    c = surf(116, 34, 0)
    a.path(f"M{n(c[0] - 9)} {n(c[1])}h18M{n(c[0])} {n(c[1] - 5)}v10", stroke=a.AD, sw=.6)
    a.ellipse(c, 6, 2.6, fill="none", stroke=a.AD, sw=.6)
    a.path(f"M{n(c[0])} {n(c[1])}V{n(c[1] - 22)}", stroke=PAL["ink"], sw=.7)
    a.path(f"M{n(c[0])} {n(c[1] - 28)}a5 5 0 1 1 .1 0Z" if False else
           f"M{n(c[0])} {n(c[1] - 16)}c-3-4-5-6-5-8.5a5 5 0 0 1 10 0c0 2.5-2 4.5-5 8.5Z", fill=a.A, stroke=PAL["ink"], sw=.5)
    a.circle((c[0], c[1] - 24.5), 1.8, fill="#fff")
    return a.render()


def roman_roads():
    """A Roman road in section and in its landscape: the large-stone statumen, the rudus and
    nucleus of rubble and lime, the cambered summa crusta of polygonal paving between kerbs, side
    ditches, a milestone, and the road's mapped line running on straight across the country."""
    a = Art("verm", seed=85)
    o = (8, 140)
    Wd, D = 196.0, 140.0
    a.shadow(o[0] + 160, o[1] + 4, 150, 7, op=.2)
    # ---- the countryside slab, cut at the front to show the road's layers
    th = 30.0
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=mix(PAL["sage1"], PAL["sand1"], .3))
    rnd = a.rng
    a.dots([ob(rnd.uniform(2, Wd - 2), rnd.uniform(2, D - 2), 0, o) for _ in range(240)], r=.35, color=PAL["moss"], op=.5)
    for k in range(4):   # field boundaries
        a.path(P([ob(0, 30 + k * 32, 0, o), ob(Wd, 22 + k * 32, 0, o)]), stroke=PAL["moss"], sw=.5, so=.6)
    rx0, rx1 = 52.0, 120.0          # road corridor across the slab (along depth)
    xm = (rx0 + rx1) / 2
    # side ditches and verges on the surface
    for xd in (rx0 - 6, rx1 + 6):
        a.path(P([ob(xd, 0, 0, o), ob(xd, D, 0, o)]), stroke=PAL["umber"], sw=2.2, so=.6)
    # paving (top of the road): polygonal stones as a tile pattern between kerbs
    ink = mix(PAL["rock"], PAL["ink"], .4)
    pave = a.pattern(9, 7, f'<path d="M0 2l3-2 4 1 2 3-1 3H3L0 5ZM4.5 3.5l2 2" fill="none" stroke="{ink}" stroke-width=".35"/>'
                     f'<path d="M7 1l2 1" stroke="{ink}" stroke-width=".35"/>', bg=PAL["rock1"])
    top = [ob(rx0, 0, 3, o), ob(rx0, D, 3, o), ob(rx1, D, 3, o), ob(rx1, 0, 3, o)]
    a.path(P(top, True), fill=pave)
    a.path(P([ob(rx1, 0, 3, o), ob(rx1, D, 3, o), ob(rx1, D, 0, o), ob(rx1, 0, 0, o)], True), fill=PAL["rock"])
    for xk in (rx0, rx1 - 2.4):       # kerbs
        a.path(P([ob(xk, 0, 4, o), ob(xk, D, 4, o), ob(xk + 2.4, D, 4, o), ob(xk + 2.4, 0, 4, o)], True), fill="#efe9dc", stroke=PAL["ink"], sw=.3)
    # ---- front cut face: the layered section
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    a.path(P([ob(0, 0, -8, o), ob(Wd, 0, -8, o), ob(Wd, 0, -th, o), ob(0, 0, -th, o)], True), fill=PAL["clay"])
    a.path(P([ob(rx0 - 4, 0, 0, o), ob(rx1 + 4, 0, 0, o), ob(rx1 + 6, 0, -25, o), ob(rx0 - 6, 0, -25, o)], True), fill=PAL["umber"])

    def xsect(z_edge, z_mid, x0, x1):
        """Cambered course top: z at edges and centre."""
        return [ob(x, 0, lerp(z_edge, z_mid, 1 - ((x - (x0 + x1) / 2) / ((x1 - x0) / 2)) ** 2), o) for x in [x0 + (x1 - x0) * k / 12 for k in range(13)]]
    courses = [  # (top edge z, top centre z, colour, pattern kind)
        (-13, -10, PAL["rockd"], "stones"), (-4, 0, PAL["sand2"], "rubble"), (1.4, 4, "#f3eee2", "fine"), (3.2, 6, PAL["rock1"], "paving")]
    bottom = [ob(x, 0, -24, o) for x in [rx0 - 2 + (rx1 - rx0 + 4) * k / 12 for k in range(13)]]
    prev = bottom
    for (ze, zm, col, kind) in courses:
        topl = xsect(ze, zm, rx0 - 2 if kind != "paving" else rx0, rx1 + 2 if kind != "paving" else rx1)
        a.path(P(topl + prev[::-1], True), fill=col)
        if kind == "stones":
            st = ""
            for k in range(9):
                p = lerp2(prev[0], prev[-1], (k + .5) / 9)
                st += f"M{n(p[0] - 3.4)} {n(p[1] - 1)}q3.4-11 6.8 0z"
            a.path(st, fill=PAL["rock1"], stroke=PAL["ink"], sw=.35)
        elif kind == "rubble":
            a.dots([lerp2(lerp2(prev[0], prev[-1], rnd.random()), lerp2(topl[0], topl[-1], rnd.random()), rnd.random() * .8) for _ in range(30)],
                   r=.6, color=PAL["ink"], op=.45)
        elif kind == "paving":
            a.path("".join(P([p, (p[0], p[1] + 2)]) for p in topl[1:-1:2]), stroke=PAL["ink"], sw=.35, so=.6)
        a.path(P(topl), stroke=PAL["ink"], sw=.4, so=.7, fill="none")
        prev = topl
    for xk in (rx0, rx1 - 2.4):
        p = ob(xk, 0, 4, o)
        a.path(f"M{n(p[0])} {n(p[1])}h2.4v10h-2.4Z", fill="#efe9dc", stroke=PAL["ink"], sw=.4)
    for xd in (rx0 - 6, rx1 + 6):     # ditches in section
        p = ob(xd, 0, 0, o)
        a.path(f"M{n(p[0] - 3)} {n(p[1])}q3 6 6 0Z", fill=PAL["water3"], stroke=PAL["ink"], sw=.4)
    slab_outline(a, f, t, r)
    # the road's mapped centre line (the dataset), running on beyond the slab
    a.path(P([ob(xm, 0, 4.6, o), ob(xm, D, 4.6, o), ob(xm, D + 40, 4.6, o)]), stroke=a.A, sw=1.2, da="5 2.4")
    # milestone and cypresses
    ms = (rx1 + 12, 46)
    cylinder(a, ms[0], ms[1], 2.0, 0, 9, o, "#efe9dc")
    for (x, y) in ((rx0 - 16, 120), (rx0 - 16, 90), (rx0 - 16, 60), (rx1 + 16, 132), (rx1 + 16, 104), (rx1 + 16, 76)):
        b = ob(x, y, 0, o)
        a.path(f"M{n(b[0])} {n(b[1] - 18)}c3 5 3.4 12 1.6 18h-3.2c-1.8-6-1.4-13 1.6-18Z", fill=PAL["forest"], stroke=PAL["forestd"], sw=.4)
    return a.render()


def county_seats():
    """A walled Chinese county seat (xian) in oblique: the square wall and moat, gate towers on
    all four sides, cross streets meeting at the drum tower, the yamen (county office) compound
    with its courtyards on the north side, and blocks of courtyard houses."""
    a = Art("verm", seed=87)
    o = (14, 164)
    Wd, D = 190.0, 170.0
    a.shadow(o[0] + 160, o[1] + 3, 150, 6, op=.18)
    f, t, r = slab(a, o, Wd, D, 5, mix(PAL["sage1"], PAL["sand1"], .3))
    # moat
    c0, c1 = 20.0, 170.0       # wall square, in x and y
    m = 8.0
    moat = [ob(c0 - m, c0 - m, 0, o), ob(c0 - m, c1 + m, 0, o), ob(c1 + m, c1 + m, 0, o), ob(c1 + m, c0 - m, 0, o)]
    inner = [ob(c0 - 3, c0 - 3, 0, o), ob(c0 - 3, c1 + 3, 0, o), ob(c1 + 3, c1 + 3, 0, o), ob(c1 + 3, c0 - 3, 0, o)]
    a.path(P(moat, True) + P(inner[::-1], True), fill=PAL["water2"], fr="evenodd", stroke=PAL["water4"], sw=.4)
    # streets
    cm = (c0 + c1) / 2
    a.path(P([ob(cm - 3, c0, 0, o), ob(cm - 3, c1, 0, o), ob(cm + 3, c1, 0, o), ob(cm + 3, c0, 0, o)], True), fill=PAL["sand1"])
    a.path(P([ob(c0, cm - 3, 0, o), ob(c1, cm - 3, 0, o), ob(c1, cm + 3, 0, o), ob(c0, cm + 3, 0, o)], True), fill=PAL["sand1"])
    tile = PAL["slate2"]
    wallc, wallt, wallr = PAL["rock1"], mix(PAL["rock1"], "#fff", .3), PAL["rock"]
    items = []   # (depth key, x key, draw function)
    rnd = a.rng
    # courtyard houses in the four quarters (skip the yamen site)
    for qx in (0, 1):
        for qy in (0, 1):
            for i in range(3):
                for j in range(3):
                    x = c0 + 8 + qx * (cm - c0) + i * 22
                    y = c0 + 8 + qy * (cm - c0) + j * 22
                    if qy == 1 and ((qx == 0 and i >= 1 and j <= 1) or (qx == 1 and i <= 1 and j <= 1)):
                        continue
                    if x + 14 > c1 - 6 or y + 14 > c1 - 6:
                        continue
                    hgt = 4.5 + rnd.random() * 1.5
                    items.append((y, x, ("house", x, y, hgt)))
    # yamen compound north of the crossing: wall, three halls on the axis
    items.append((cm + 10, cm - 22, ("yamen",)))
    for (key, xk, it) in sorted(items, key=lambda q: (-q[0], q[1])):
        if it[0] == "house":
            _, x, y, hgt = it
            box3(a, x, x + 15, y, y + 11, 0, hgt, o, PAL["paper2"], None, mix(PAL["paper2"], PAL["ink"], .15), sw=.25)
            hiproof(a, x, x + 15, y, y + 11, hgt, 4, o, tile, overhang=1)
        else:
            yx0, yx1, yy0, yy1 = cm - 24, cm + 24, cm + 8, cm + 66
            box3(a, yx0, yx1, yy0, yy1, 0, 3, o, a.A, None, a.AD, sw=.3)     # vermilion compound wall
            a.path(P([ob(yx0 + 1, yy0 + 1, 3, o), ob(yx0 + 1, yy1 - 1, 3, o), ob(yx1 - 1, yy1 - 1, 3, o), ob(yx1 - 1, yy0 + 1, 3, o)], True), fill="#efe8da")
            for (y0, y1, hh, w) in ((yy1 - 18, yy1 - 6, 9, 22), (yy0 + 26, yy0 + 36, 8, 18), (yy0 + 6, yy0 + 14, 6, 12)):
                box3(a, cm - w, cm + w, y0, y1, 0, hh, o, a.A, None, a.AD, sw=.3)
                hiproof(a, cm - w, cm + w, y0, y1, hh, 5, o, tile)
    # walls: back and left first, then right and front, each with crenellations
    wt, wh = 4.0, 8.0
    box3(a, c0, c1, c1 - wt, c1, 0, wh, o, wallc, wallt, wallr, sw=.35)
    box3(a, c0, c0 + wt, c0, c1, 0, wh, o, wallc, wallt, wallr, sw=.35)
    box3(a, c1 - wt, c1, c0, c1, 0, wh, o, wallc, wallt, wallr, sw=.35)
    box3(a, c0, c1, c0, c0 + wt, 0, wh, o, wallc, wallt, wallr, sw=.35)
    merlons(a, [(c0, c0), (c1, c0)], wh, o, every=4)
    merlons(a, [(c1, c0), (c1, c1)], wh, o, every=4)
    # drum tower at the crossing
    box3(a, cm - 7, cm + 7, cm - 7, cm + 7, 0, 10, o, wallc, None, wallr, sw=.35)
    box3(a, cm - 5, cm + 5, cm - 5, cm + 5, 10, 16, o, a.A, None, a.AD, sw=.3)
    hiproof(a, cm - 5, cm + 5, cm - 5, cm + 5, 16, 5, o, tile, overhang=2.4)
    # gate towers over the four gates (back first)
    for (gx, gy) in ((cm, c1 - wt / 2), (c0 + wt / 2, cm), (c1 - wt / 2, cm), (cm, c0 + wt / 2)):
        box3(a, gx - 7, gx + 7, gy - 4, gy + 4, wh, wh + 7, o, a.A, None, a.AD, sw=.3)
        hiproof(a, gx - 7, gx + 7, gy - 4, gy + 4, wh + 7, 5, o, tile, overhang=2.2)
    gp = ob(cm, c0, 0, o)
    a.path(f"M{n(gp[0] - 3)} {n(gp[1])}v-4.4a3 3 0 0 1 6 0v4.4Z", fill=PAL["slated"])
    slab_outline(a, f, t, r)
    return a.render()


def temples():
    """A Chinese Buddhist temple laid out on its north-south axis: the mountain gate, the bell and
    drum towers, the Hall of the Heavenly Kings, the double-eaved Mahavira Hall and a pagoda
    behind, all within a vermilion-walled precinct among trees."""
    a = Art("verm", seed=89)
    o = (16, 165)
    Wd, D = 190.0, 170.0
    a.shadow(o[0] + 160, o[1] + 3, 150, 6, op=.18)
    f, t, r = slab(a, o, Wd, D, 5, mix(PAL["sage1"], PAL["sand1"], .25))
    tile = PAL["slate2"]
    x0, x1, y0, y1 = 40.0, 150.0, 12.0, 160.0
    xm = (x0 + x1) / 2
    a.path(P([ob(x0, y0, 0, o), ob(x0, y1, 0, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)], True), fill="#ece5d6")
    a.path(P([ob(xm - 4, y0, 0, o), ob(xm - 4, y1 - 20, 0, o), ob(xm + 4, y1 - 20, 0, o), ob(xm + 4, y0, 0, o)], True), fill=PAL["paper"],
           stroke=PAL["ink"], sw=.25, so=.4)
    # trees outside the precinct
    rnd = a.rng
    for (x, y) in sorted([(rnd.uniform(4, 34), rnd.uniform(10, 166)) for _ in range(7)] +
                         [(rnd.uniform(156, 186), rnd.uniform(10, 166)) for _ in range(7)], key=lambda p: -p[1]):
        b = ob(x, y, 0, o)
        a.path(broadleaf(b[0], b[1], 11, seed=int(x + y)), fill=PAL["moss"], stroke=PAL["forest"], sw=.4)
        a.path(f"M{n(b[0])} {n(b[1])}v-4", stroke=PAL["umber"], sw=.8)
    wall, walld = a.A, a.AD
    box3(a, x0, x1, y1 - 2, y1, 0, 6, o, wall, mix(PAL["slate1"], "#fff", .2), walld, sw=.3)
    box3(a, x0, x0 + 2, y0, y1, 0, 6, o, wall, mix(PAL["slate1"], "#fff", .2), walld, sw=.3)
    # pagoda (back), then halls from back to front, bell and drum towers
    px, py = xm, 140
    zc = 0.0
    for k in range(5):
        s = 9 - k * 1.2
        hh = 7 if k == 0 else 5
        box3(a, px - s * .7, px + s * .7, py - s * .7, py + s * .7, zc, zc + hh, o, "#efe8da", None, mix("#efe8da", PAL["ink"], .15), sw=.3)
        zc += hh
        hiproof(a, px - s * .7, px + s * .7, py - s * .7, py + s * .7, zc, 2.2, o, tile, inset=s * .55, overhang=2.6)
        zc += 1.0
    tip = ob(px, py, zc + 1, o)
    a.path(f"M{n(tip[0])} {n(tip[1] + 1)}v-9", stroke=PAL["ink"], sw=.8)
    a.path("".join(f"M{n(tip[0] - 1.6)} {n(tip[1] - 2 - k * 2)}h3.2" for k in range(4)), stroke=PAL["ink"], sw=.6)
    halls = [(110, 20, 13, 13, True), (72, 16, 9, 9, False), (34, 12, 7, 7, False)]   # (y, half width, depth, height, double eave)
    for (y, hw, dp, hh, dbl) in halls:
        box3(a, xm - hw, xm + hw, y - dp / 2, y + dp / 2, 0, 1.6, o, PAL["paper2"], "#faf6ee", PAL["rock1"], sw=.3)   # platform
        box3(a, xm - hw + 2, xm + hw - 2, y - dp / 2 + 1.5, y + dp / 2 - 1.5, 1.6, 1.6 + hh, o, wall, None, walld, sw=.3)
        cols = "".join(P([ob(xm - hw + 2 + k * (2 * hw - 4) / 6, y - dp / 2 + 1.5, 1.6, o), ob(xm - hw + 2 + k * (2 * hw - 4) / 6, y - dp / 2 + 1.5, 1.6 + hh, o)]) for k in range(7))
        a.path(cols, stroke=walld, sw=.6, so=.8)
        if dbl:
            hiproof(a, xm - hw + 2, xm + hw - 2, y - dp / 2 + 1.5, y + dp / 2 - 1.5, 1.6 + hh, 3, o, tile, inset=0, overhang=3)
            box3(a, xm - hw + 6, xm + hw - 6, y - dp / 2 + 3.5, y + dp / 2 - 3.5, 4.6 + hh, 8.6 + hh, o, wall, None, walld, sw=.3)
            hiproof(a, xm - hw + 6, xm + hw - 6, y - dp / 2 + 3.5, y + dp / 2 - 3.5, 8.6 + hh, 7, o, tile, overhang=3)
        else:
            hiproof(a, xm - hw + 2, xm + hw - 2, y - dp / 2 + 1.5, y + dp / 2 - 1.5, 1.6 + hh, 5.5, o, tile, overhang=2.4)
    for xt in (x0 + 14, x1 - 14):   # bell tower and drum tower
        box3(a, xt - 5, xt + 5, 50, 60, 0, 9, o, PAL["paper2"], None, PAL["rock1"], sw=.3)
        box3(a, xt - 4, xt + 4, 51, 59, 9, 14, o, wall, None, walld, sw=.3)
        hiproof(a, xt - 4, xt + 4, 51, 59, 14, 4, o, tile, overhang=2.2)
    # right and front walls, the mountain gate in the front wall
    box3(a, x1 - 2, x1, y0, y1, 0, 6, o, wall, mix(PAL["slate1"], "#fff", .2), walld, sw=.3)
    box3(a, x0, xm - 12, y0, y0 + 2, 0, 6, o, wall, mix(PAL["slate1"], "#fff", .2), walld, sw=.3)
    box3(a, xm + 12, x1, y0, y0 + 2, 0, 6, o, wall, mix(PAL["slate1"], "#fff", .2), walld, sw=.3)
    box3(a, xm - 12, xm + 12, y0 - 2, y0 + 4, 0, 9, o, wall, None, walld, sw=.3)
    for dx in (-6, 0, 6):
        g = ob(xm + dx, y0 - 2, 0, o)
        a.path(f"M{n(g[0] - 1.8)} {n(g[1])}v-4.2a1.8 1.8 0 0 1 3.6 0v4.2Z", fill=PAL["slated"])
    hiproof(a, xm - 12, xm + 12, y0 - 2, y0 + 4, 9, 4.4, o, tile, overhang=2.4)
    slab_outline(a, f, t, r)
    return a.render()


def castles():
    """A concentric castle on its hill: moat, an outer curtain wall with round flanking towers and
    a twin-towered gatehouse, a higher inner ward and the square keep with corner turrets, all
    battlemented, with the near wall cut away to show its thickness."""
    a = Art("verm", seed=91)
    o = (12, 160)
    Wd, D = 214.0, 124.0

    def h(x, y):
        return 6 + 16 * math.exp(-((x - 104) ** 2 / 3600 + (y - 62) ** 2 / 2400))
    a.shadow(o[0] + 160, o[1] + 4, 150, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=24, ny=10, colours={"land": PAL["sage2"]}, base=12,
                   strata=[(2.5, PAL["umber"]), (6, PAL["clay"]), (99, PAL["rock"])])
    cx, cy = 104.0, 62.0
    zc = h(cx, cy) - 1
    # moat ring on the hillside
    ring = lambda rx, ry, dz=.3: [surf(cx + rx * math.cos(t), cy + ry * math.sin(t), dz) for t in [2 * math.pi * i / 48 for i in range(48)]]
    a.path(P(ring(80, 52), True) + P(ring(72, 46)[::-1], True), fill=PAL["water2"], fr="evenodd", stroke=PAL["water4"], sw=.4)
    stone, stone_t, stone_d = "#ece6da", "#f7f3ea", PAL["rock1"]
    # outer curtain: a rectangle of walls with round towers at the corners and mid-sides
    ox0, ox1, oy0, oy1 = cx - 56, cx + 56, cy - 34, cy + 34
    wz, wh, wt = zc - 2, 10.0, 3.5
    box3(a, ox0, ox1, oy1 - wt, oy1, wz, wz + wh, o, stone, stone_t, stone_d)
    box3(a, ox0, ox0 + wt, oy0, oy1, wz, wz + wh, o, stone, stone_t, stone_d)
    for (tx, ty) in ((ox0, oy1), (cx, oy1), (ox1, oy1)):
        cylinder(a, tx, ty, 6, wz, wz + wh + 5, o, stone)
    # inner ward: higher walls and the keep
    ix0, ix1, iy0, iy1 = cx - 30, cx + 30, cy - 18, cy + 18
    iz, ih = zc, 15.0
    box3(a, ix0, ix1, iy1 - wt, iy1, iz, iz + ih, o, stone, stone_t, stone_d)
    box3(a, ix0, ix0 + wt, iy0, iy1, iz, iz + ih, o, stone, stone_t, stone_d)
    for (tx, ty) in ((ix0, iy1), (ix1, iy1)):
        cylinder(a, tx, ty, 5, iz, iz + ih + 5, o, stone)
    # the keep with corner turrets and a pennant
    kx0, kx1, ky0, ky1 = cx - 11, cx + 11, cy - 9, cy + 9
    kz = iz + 32
    box3(a, kx0, kx1, ky0, ky1, iz, kz, o, stone, stone_t, stone_d)
    win = "".join(f"M{n(ob(kx0 + 4 + k * 7, ky0, kz - 8 - j * 9, o)[0])} {n(ob(kx0 + 4 + k * 7, ky0, kz - 8 - j * 9, o)[1])}v2.6" for k in range(3) for j in range(2))
    a.path(win, stroke=PAL["ink"], sw=1.0)
    merlons(a, [(kx0, ky0), (kx1, ky0), (kx1, ky1)], kz, o, every=3)
    for (tx, ty) in ((kx0, ky1), (kx1, ky1), (kx0, ky0), (kx1, ky0)):
        cylinder(a, tx, ty, 3, kz - 4, kz + 5, o, stone)
    fp = ob(kx1, ky1, kz + 5, o)
    a.path(f"M{n(fp[0])} {n(fp[1])}v-12", stroke=PAL["ink"], sw=.6)
    a.path(f"M{n(fp[0])} {n(fp[1] - 12)}l9 2.4-9 2.4Z", fill=a.A)
    # inner ward right and front walls, with towers
    box3(a, ix1 - wt, ix1, iy0, iy1, iz, iz + ih, o, stone, stone_t, stone_d)
    box3(a, ix0, ix1, iy0, iy0 + wt, iz, iz + ih, o, stone, stone_t, stone_d)
    merlons(a, [(ix0, iy0), (ix1, iy0), (ix1, iy1)], iz + ih, o, every=3)
    for (tx, ty) in ((ix0, iy0), (ix1, iy0)):
        cylinder(a, tx, ty, 5, iz, iz + ih + 5, o, stone)
    # outer right wall, then the front wall cut away at the gatehouse
    box3(a, ox1 - wt, ox1, oy0, oy1, wz, wz + wh, o, stone, stone_t, stone_d)
    box3(a, ox0, cx - 9, oy0, oy0 + wt, wz, wz + wh, o, stone, stone_t, stone_d)
    box3(a, cx + 9, ox1, oy0, oy0 + wt, wz, wz + wh, o, stone, stone_t, stone_d)
    # rubble core showing in the cut ends of the front wall
    for xe in (cx - 9, cx + 9):
        p = ob(xe, oy0, wz + wh, o)
    merlons(a, [(ox0, oy0), (cx - 9, oy0)], wz + wh, o, every=3)
    merlons(a, [(cx + 9, oy0), (ox1, oy0), (ox1, oy1)], wz + wh, o, every=3)
    for (tx, ty) in ((ox0, oy0), (ox1, oy0), (ox1, cy)):
        cylinder(a, tx, ty, 6, wz, wz + wh + 5, o, stone)
    # gatehouse: two drum towers flanking the gate passage, a bridge over the moat
    for gx in (cx - 9, cx + 9):
        cylinder(a, gx, oy0, 6.5, wz, wz + wh + 9, o, stone)
    g = ob(cx, oy0 - 2, wz, o)
    a.path(f"M{n(g[0] - 3.2)} {n(g[1])}v-6a3.2 3.2 0 0 1 6.4 0v6Z", fill=PAL["slated"])
    b0, b1 = surf(cx, oy0 - 18, .5), ob(cx, oy0 - 3, wz, o)
    a.path(P([(b0[0] - 3, b0[1]), (b1[0] - 3, b1[1]), (b1[0] + 3, b1[1]), (b0[0] + 3, b0[1])], True), fill=PAL["clay"], stroke=PAL["ink"], sw=.4)
    return a.render()
