"""History: World Heritage, ancient places, Roman roads, county seats, temples, castles."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, Equi, P, S, circ, ell, ell_pts, rect, scallop,
                  lerp, lerp2, mix, cities)


def crenel(x0, x1, y, mw=2.6, mh=2.4):
    """Top edge of a battlement from x0 to x1 at wall-top y: merlons and crenels."""
    pts = [(x0, y)]
    x = x0
    up = True
    while x < x1 - 0.1:
        nx = min(x + mw, x1)
        if up:
            pts += [(x, y - mh), (nx, y - mh), (nx, y)]
        else:
            pts += [(nx, y)]
        x = nx
        up = not up
    return pts


# ------------------------------------------------------------------ World Heritage
def heritage():
    pg = Page(17)
    # a steep peak behind a saddle with terraced stone ruins (a mixed cultural and natural site)
    peak = [(110, 132), (128, 70), (138, 40), (146, 30), (154, 38), (164, 66), (176, 104), (196, 132)]
    pg.wash(S(peak, True, 0.3), C["hooker"], 0.55)
    pg.wash(S([(146, 30), (154, 38), (164, 66), (176, 104), (196, 132), (160, 132), (152, 80)], True), C["payne"], 0.35,
            edge=0)
    pg.ink(S(peak, False, 0.3), 0.75)
    pg.hatch([(150, 40), (160, 66), (170, 100), (160, 104), (152, 70)], -60, 2.2, 0.35, INK2, 0.7)
    ridge = [(4, 120), (40, 104), (80, 96), (112, 100), (130, 116), (150, 128), (212, 136), (212, 170), (4, 170)]
    pg.paper(S(ridge, True, 0.3))
    pg.wash(S(ridge, True, 0.3), C["sapl"], 0.65, edge=0)
    pg.ink(S(ridge[:7], False, 0.3), 0.7)
    # cloud in the valley
    cl = [(160, 120), (176, 112), (196, 114), (210, 120)]
    pg.wash(S(cl + [(210, 126), (160, 126)], True), C["white"], 0.95, edge=0)
    pg.ink(scallop(cl, 0.35), 0.5, INK2)
    # agricultural terraces stepping down the slope
    for k in range(6):
        y = 120 + k * 8
        pg.ink(S([(8 + k * 3, y - 6), (40, y - 10 - k), (80, y - 10), (120, y - 4)]), 0.5, INK2)
        pg.wash(S([(8 + k * 3, y - 6), (40, y - 10 - k), (80, y - 10), (120, y - 4), (120, y - 2.5), (80, y - 8.5),
                   (40, y - 8.5 - k), (8 + k * 3, y - 4.5)], True), C["umberl"], 0.5, edge=0)
    # roofless stone houses on the saddle: gable walls with doorways
    for x, y, w in ((52, 100, 12), (68, 97, 10), (84, 98, 13), (100, 101, 10), (60, 108, 12), (78, 107, 11)):
        house = [(x, y), (x, y - 6), (x + w * 0.5, y - 10), (x + w, y - 6), (x + w, y)]
        pg.wash(P(house, True), C["greyl"], 0.9, edge=0.5)
        pg.ink(P(house, True) + rect(x + w * 0.4, y - 4, w * 0.2, 4), 0.5)
    # insets: the two kinds of heritage, cultural (a column capital) and natural (a mountain flower)
    with pg.inset_circle(268, 48, 32):
        pg.wash(circ(268, 48, 32), C["skyp"], 0.35, edge=0)
        pg.wash(rect(258, 48, 20, 34), C["greyl"], 0.8, edge=0)
        pg.ink("".join(P([(x, 48), (x, 82)]) for x in (258, 262, 266, 270, 274, 278)), 0.45)
        pg.wash(rect(246, 34, 44, 7), C["greyl"], 0.9, edge=0)
        pg.ink(rect(246, 34, 44, 7) + P([(250, 41), (286, 41)]), 0.6)
        for sx in (-1, 1):                         # volutes of an Ionic capital
            cx = 268 + sx * 16
            spiral = [(cx + math.cos(a) * (6 - a * 0.55) * sx, 45 + math.sin(a) * (6 - a * 0.55)) for a in
                      [i * 0.45 for i in range(19)]]
            pg.ink(S(spiral), 0.6)
        pg.ink(P([(254, 46), (282, 46)]) + P([(256, 48), (280, 48)]), 0.5)
    with pg.inset_circle(268, 130, 32):
        pg.wash(circ(268, 130, 32), C["skyp"], 0.35, edge=0)
        pg.ink(S([(268, 160), (266, 140), (268, 124)]), 0.7, C["hooker"])
        for a in range(5):
            ang = math.radians(-90 + a * 72)
            px, py = 268 + math.cos(ang) * 8, 118 + math.sin(ang) * 8
            pet = S([(268, 118), (268 + math.cos(ang - .45) * 9, 118 + math.sin(ang - .45) * 9), (px + math.cos(ang) * 3,
                     py + math.sin(ang) * 3), (268 + math.cos(ang + .45) * 9, 118 + math.sin(ang + .45) * 9)], True)
            pg.wash(pet, C["violet"], 0.75)
            pg.ink(pet, 0.5)
        pg.wash(circ(268, 118, 2.4), C["lemon"], 0.95, edge=0)
        for sx in (-1, 1):
            leaf = [(267, 146), (267 + sx * 10, 138), (267 + sx * 16, 140), (267 + sx * 8, 148)]
            pg.wash(S(leaf, True), C["sap"], 0.75)
            pg.ink(S(leaf, True), 0.5)
    pg.leader((84, 92), (238, 54), bend=-0.06)
    pg.leader((146, 34), (240, 122), bend=0.1)
    return pg.svg()


# ------------------------------------------------------------------ ancient places
def ancient_places():
    pg = Page(18)
    hz = 70
    pg.vignette(4, 24, 212, hz, C["skyp"], 0.4, 3, seed=1)
    pg.wash(rect(4, hz, 208, 10), C["cerul"], 0.5, edge=0)
    pg.ink(P([(4, hz), (212, hz)]), 0.5, INK2)
    hill = [(4, 104), (50, 92), (110, 90), (170, 94), (212, 100), (212, 168), (4, 168)]
    pg.wash(S(hill, True, 0.3), C["ochrel"], 0.6, edge=0)
    pg.ink(S(hill[:5]), 0.6)
    # a Doric temple, partly standing: stepped base (stylobate), columns carrying a stretch of architrave
    x0, x1, sty = 40, 176, 112
    for k in range(3):
        pg.wash(rect(x0 - 6 + k * 3, sty + 6 - k * 3, x1 - x0 + 12 - k * 6, 3), C["sand"], 0.9, edge=0.4)
        pg.ink(rect(x0 - 6 + k * 3, sty + 6 - k * 3, x1 - x0 + 12 - k * 6, 3), 0.5)
    col_h = 40
    xs = [x0 + 4 + i * 16.5 for i in range(9)]
    standing = [0, 1, 2, 3, 6, 8]
    heights = {0: col_h, 1: col_h, 2: col_h, 3: col_h, 6: 22, 8: 12}
    for i in standing:
        x = xs[i]
        h = heights[i]
        top = sty - h
        shaft = [(x - 4, sty), (x - 3.3, top + (5 if h == col_h else 0)), (x + 3.3, top + (5 if h == col_h else 0)), (x + 4, sty)]
        pg.wash(P(shaft, True), C["white"], 0.9, edge=0)
        pg.wash(P([(x + 1, sty), (x + 1, top + 5), (x + 3.3, top + 5), (x + 4, sty)], True), C["umberl"], 0.4, edge=0)
        pg.ink(P(shaft[:2]) + P(shaft[2:]) + P([(x, sty), (x, top + 5)]), 0.5)
        pg.ink(P([(x - 1.8, sty), (x - 1.7, top + 5)]) + P([(x + 1.8, sty), (x + 1.7, top + 5)]), 0.25, INK2)
        if h == col_h:
            # echinus and abacus: the capital
            pg.ink(P([(x - 3.3, top + 5), (x - 5, top + 2), (x + 5, top + 2), (x + 3.3, top + 5)]) +
                   rect(x - 5.4, top, 10.8, 2), 0.55)
        else:
            pg.ink(P([(x - 3.6, top), (x - 1, top - 1.2), (x + 1.5, top + 0.6), (x + 3.6, top - .4)]), 0.5)  # broken top
    # the surviving architrave rests on the four full columns
    ax0, ax1 = xs[0] - 5.4, xs[3] + 5.4
    at = sty - col_h - 7
    pg.wash(rect(ax0, at, ax1 - ax0, 7), C["sand"], 0.9, edge=0.4)
    pg.ink(rect(ax0, at, ax1 - ax0, 7) + P([(ax0, at + 3), (ax1, at + 3)]), 0.55)
    # the frieze above it: triglyphs (grooved blocks) alternating with plain metopes
    pg.wash(rect(ax0, at - 7, ax1 - ax0, 7), C["sand"], 0.8, edge=0.4)
    pg.ink(rect(ax0, at - 7, ax1 - ax0, 7), 0.5)
    tri = ""
    for k in range(7):
        tx_ = ax0 + 1 + k * (ax1 - ax0 - 5) / 6
        tri += rect(tx_, at - 7, 3.4, 7) + P([(tx_ + 1.1, at - 6), (tx_ + 1.1, at - 1)]) + P([(tx_ + 2.3, at - 6), (tx_ + 2.3, at - 1)])
    pg.ink(tri, 0.35)
    # fallen drums on the ground
    for x, y in ((140, 128), (152, 132), (164, 129), (186, 136)):
        pg.wash(ell(x, y, 3, 4), C["white"], 0.9, edge=0)
        pg.wash(P([(x, y - 4), (x + 9, y - 4.6), (x + 9, y + 3.4), (x, y + 4)], True), C["sand"], 0.85, edge=0)
        pg.ink(ell(x, y, 3, 4) + P([(x, y - 4), (x + 9, y - 4.6)]) + P([(x, y + 4), (x + 9, y + 3.4)]), 0.5)
    # an olive tree
    pg.ink(S([(24, 136), (22, 124), (26, 112)]) + S([(22, 124), (16, 116)]), 0.9, C["umber"])
    ol = [(8, 112), (12, 102), (24, 96), (36, 100), (40, 110), (30, 116), (16, 118)]
    pg.wash(S(ol, True), C["olive"], 0.6)
    pg.ink(scallop(ol, 0.25, True), 0.5, INK2)
    # inset: top of a fluted drum (twenty flutes) and a map of places
    with pg.inset_circle(270, 50, 30):
        pg.wash(circ(270, 50, 30), C["skyp"], 0.35, edge=0)
        pts = []
        for i in range(40):
            a = i * math.pi / 20
            r = 20 if i % 2 == 0 else 18.6
            pts.append((270 + math.cos(a) * r, 50 + math.sin(a) * r))
        pg.wash(P(pts, True), C["sand"], 0.85)
        pg.ink(S(pts, True, 0.2), 0.55)
        pg.ink(circ(270, 50, 2), 0.5)                       # the dowel hole
    pg.leader((xs[6], sty - 22), (240, 54), bend=-0.1)
    m = Equi(19, 30, 35, 41.5, 230, 98, 82)
    with pg.inset_box(230, 98, 82, m.h):
        pg.wash(rect(230, 98, 82, m.h), C["sky"], 0.5, edge=0)
        pg.land(m.land(tol=0.8), C["sand"], 0.9)
        rng = random.Random(18)
        dots = []
        for lon, lat in [(23.73, 37.97), (22.0, 37.6), (22.5, 38.5), (21.6, 37.6), (25.4, 37.4), (24.9, 35.3), (25.2, 35.3),
                         (22.9, 40.6), (23.3, 38.2), (26.2, 39.1), (27.3, 37.9), (27.2, 37.0), (28.0, 36.4), (22.5, 37.7),
                         (24.4, 40.9), (21.4, 39.4), (25.6, 36.4), (22.4, 37.0), (26.0, 38.4), (20.7, 39.6)]:
            x, y = m(lon, lat)
            dots.append((x, y))
        pg.dots(dots, 1.1, RED_PENCIL)
    return pg.svg()


# ------------------------------------------------------------------ Roman roads
def roman_roads():
    pg = Page(19)
    hz = 46
    vp = (128, hz)
    pg.vignette(4, 12, 212, hz, C["skyp"], 0.35, 3, seed=1)
    hills = [(4, hz + 2), (40, hz - 6), (90, hz - 2), (150, hz - 8), (212, hz)]
    pg.ink(S(hills), 0.5, INK2)
    pg.vignette(4, hz, 212, 104, C["sapl"], 0.5, 3, seed=2)
    # a dead-straight road to the horizon
    road = [(36, 104), (104, 104), vp]
    pg.wash(P([(40, 104), (100, 104), (129, hz + 0.5), (127, hz + 0.5)], True), C["greyl"], 0.85)
    pg.ink(P([(40, 104), (127, hz + .5)]) + P([(100, 104), (129, hz + .5)]), 0.6)
    pg.ink(P([(34, 104), (126.5, hz + .5)]) + P([(106, 104), (129.5, hz + .5)]), 0.35, INK2, 0.8, "2 2")   # ditches
    rng = random.Random(19)
    st = ""
    for k in range(18):
        t = (k / 18) ** 1.6
        y = lerp(104, hz, t)
        xl, xr = lerp(40, 127, t), lerp(100, 129, t)
        st += P([(xl, y), (xr, y)])
    pg.ink(st, 0.25, INK2, 0.6)
    # a milestone and a row of cypresses
    pg.wash(P([(116, 102), (116, 86), (118, 84), (122, 84), (124, 86), (124, 102)], True), C["greyl"], 0.9, edge=0.5)
    pg.ink(P([(116, 102), (116, 86), (118, 84), (122, 84), (124, 86), (124, 102)]), 0.6)
    for x, h in ((150, 30), (162, 24), (172, 19), (180, 15)):
        cy = [(x, hz + 46 - (150 - x) * 0.6 - h), (x - 3, hz + 40 - (150 - x) * 0.6 - h * 0.4), (x, hz + 46 - (150 - x) * 0.6),
              (x + 3, hz + 40 - (150 - x) * 0.6 - h * 0.4)]
        pg.wash(S(cy, True), C["hooker"], 0.75)
        pg.ink(S(cy, True), 0.5)
    # the section through the road: paving, fine concrete, rubble, foundation stones, kerbs and ditches
    bx, by, bw, bh = 12, 112, 196, 58
    with pg.inset_box(bx, by, bw, bh):
        pg.wash(rect(bx, by, bw, bh), C["umberl"], 0.3, edge=0)
        L, R = 44, 176
        top = by + 14

        def layer(y0, y1, color, op=0.75):
            pg.wash(P([(L, y0), (R, y0), (R, y1), (L, y1)], True), color, op, edge=0)
        layer(top + 22, top + 34, C["grey"])                      # statumen: big stones
        layer(top + 12, top + 22, C["sand"])                      # rudus: rubble in lime
        layer(top + 6, top + 12, C["ochrel"])                     # nucleus: fine concrete
        # summa crusta: polygonal paving blocks on a camber
        crown = [(L, top + 6), ((L + R) / 2, top), (R, top + 6)]
        pg.wash(P(crown + [(R, top + 8), (L, top + 8)], True), C["slate"], 0.7, edge=0)
        pg.ink(P(crown) + "".join(P([(x, top + 6 - 6 * (1 - abs(x - (L + R) / 2) / ((R - L) / 2))), (x + 3, top + 8)])
                                  for x in range(L + 10, R - 4, 13)), 0.5)
        for y in (top + 6, top + 12, top + 22, top + 34):
            pg.ink(P([(L, y), (R, y)]), 0.45)
        stones = "".join(ell(x, top + 28 + (i % 2) * 1.5, 4.6, 3.4) for i, x in enumerate(range(L + 6, R - 2, 10)))
        pg.ink(stones, 0.4)
        pg.ink("".join(circ(x, top + 17 + (i % 3), 1.2) for i, x in enumerate(range(L + 4, R, 6))), 0.3)
        # kerbstones and side ditches
        for kx in (L - 6, R):
            pg.wash(rect(kx, top - 2, 6, 14), C["greyl"], 0.9)
            pg.ink(rect(kx, top - 2, 6, 14), 0.55)
        for dx in (L - 20, R + 14):
            pg.ink(S([(dx - 8, top + 4), (dx, top + 12), (dx + 8, top + 4)]), 0.6)
            pg.wash(S([(dx - 5, top + 8), (dx, top + 11), (dx + 5, top + 8)], True), C["cerul"], 0.7, edge=0)
        pg.ink(P([(bx, top + 4), (L - 28, top + 4)]) + P([(L - 12, top + 4), (L - 6, top + 4)]) +
               P([(R + 6, top + 4), (R + 6, top + 4)]) + P([(R + 22, top + 4), (bx + bw, top + 4)]), 0.6)
        pg.arrow([(R - 20, top - 6), (R - 4, top - 2)], BLUE_ARROW, 0.7, 2.2)          # rain runs off the camber
        pg.arrow([(L + 20, top - 6), (L + 4, top - 2)], BLUE_ARROW, 0.7, 2.2)
    pg.leader((70, 100), (70, by), dot_b=False)
    # the network, radiating from Rome across Italy
    m = Equi(6, 20, 36.5, 46.5, 230, 18, 82)
    with pg.inset_box(230, 18, 82, m.h):
        pg.wash(rect(230, 18, 82, m.h), C["sky"], 0.5, edge=0)
        pg.land(m.land(tol=0.8), C["sand"], 0.9)
        rome = m(12.5, 41.9)
        d = ""
        for lon, lat in ((9.2, 45.5), (11.3, 44.5), (12.6, 44.06), (16.9, 41.1), (18.0, 40.6), (15.6, 38.1), (14.3, 40.85),
                         (13.4, 42.35), (10.4, 43.7), (8.9, 44.4)):
            d += P([rome, m(lon, lat)])
        pg.ink(d, 0.6, RED_PENCIL, 0.95)
        pg.dot(rome[0], rome[1], 1.5)
    return pg.svg()


# ------------------------------------------------------------------ county seats (imperial China)
def county_seats():
    pg = Page(20)
    # a walled county town in a quick oblique sketch: moat, square wall, four gates, crossing streets
    def O(x, y):          # plan (x right, y back) -> screen, oblique
        return (24 + x + y * 0.45, 160 - y * 0.9)
    W_ = 120
    moat_o = [O(-10, -10), O(W_ + 10, -10), O(W_ + 10, W_ + 10), O(-10, W_ + 10)]
    moat_i = [O(-4, -4), O(W_ + 4, -4), O(W_ + 4, W_ + 4), O(-4, W_ + 4)]
    pg.wash(P(moat_o, True) + P(moat_i[::-1], True), C["cerul"], 0.6, edge=0)
    pg.ink(P(moat_o, True) + P(moat_i, True), 0.45, INK2)
    pg.wash(P([O(0, 0), O(W_, 0), O(W_, W_), O(0, W_)], True), C["ochrel"], 0.45, edge=0)
    # streets: the cross through the gates
    pg.ink(P([O(W_ / 2, 0), O(W_ / 2, W_)]) + P([O(0, W_ / 2), O(W_, W_ / 2)]), 1.6, C["umberl"], 0.8)
    # houses in blocks
    rng = random.Random(20)
    for bx in (8, 68):
        for byy in (8, 68):
            for i in range(3):
                for j in range(3):
                    if (bx, byy) == (68, 68) and i < 2 and j < 2:
                        continue
                    x, y = bx + i * 15, byy + j * 15
                    a, b, c_, d_ = O(x, y), O(x + 10, y), O(x + 10, y + 8), O(x, y + 8)
                    pg.wash(P([a, b, c_, d_], True), rng.choice([C["slate"], C["greyl"], C["payne"]]), 0.55, edge=0)
                    pg.ink(P([a, b, c_, d_], True), 0.3, INK2)
    # the yamen (magistrate's compound) in the north-east block, with its hall
    ya = [O(70, 70), O(100, 70), O(100, 100), O(70, 100)]
    pg.wash(P(ya, True), C["red"], 0.35, edge=0)
    pg.ink(P(ya, True), 0.6, C["bsienna"])
    hall = [O(78, 82), O(92, 82), O(92, 90), O(78, 90)]
    pg.wash(P(hall, True), C["crimson"], 0.7, edge=0)
    pg.ink(P(hall, True), 0.5)
    # walls with crenellated tops (drawn as thick lines) and gate towers
    wall = [O(0, 0), O(W_, 0), O(W_, W_), O(0, W_), O(0, 0)]
    pg.ink(P(wall), 2.4, C["umber"], 0.85)
    pg.ink(P(wall), 0.5, INK)
    for gx, gy_ in ((W_ / 2, 0), (W_ / 2, W_), (0, W_ / 2), (W_, W_ / 2)):
        x, y = O(gx, gy_)
        pg.wash(rect(x - 6, y - 7, 12, 6), C["crimson"], 0.75, edge=0)
        roof = [(x - 8, y - 7), (x - 5, y - 11), (x + 5, y - 11), (x + 8, y - 7)]
        pg.wash(P(roof, True), C["payne"], 0.8, edge=0)
        pg.ink(rect(x - 6, y - 7, 12, 6) + P(roof, True), 0.5)
    # timeline: the seat was founded and later abolished (BEG_YR, END_YR) among the dynasties
    x0, x1, ty = 222, 312, 30
    segs = [(0, 0.14, C["ochrel"]), (0.14, 0.33, C["sapl"]), (0.33, 0.47, C["sky"]), (0.47, 0.62, C["rose"]),
            (0.62, 0.8, C["sand"]), (0.8, 1.0, C["violet"])]
    for a, b, c in segs:
        pg.wash(rect(lerp(x0, x1, a), ty, (x1 - x0) * (b - a), 8), c, 0.8, edge=0)
    pg.ink(rect(x0, ty, x1 - x0, 8) + "".join(P([(lerp(x0, x1, a), ty), (lerp(x0, x1, a), ty + 8)]) for a, _, _ in segs[1:]),
           0.45)
    fb, fe = lerp(x0, x1, 0.38), lerp(x0, x1, 0.71)
    pg.ink(P([(fb, ty - 6), (fb, ty + 14)]) + P([(fe, ty - 6), (fe, ty + 14)]), 0.8, RED_PENCIL)
    pg.arrow([(fb, ty + 18), (fe, ty + 18)], RED_PENCIL, 0.8, 2.4, smooth=False)
    pg.arrow([(x0, ty - 10), (x1, ty - 10)], INK, 0.5, 2.0, smooth=False)
    # a gate tower in elevation: a two-eaved tower on the wall over an arched gateway
    with pg.inset_circle(270, 118, 40):
        pg.wash(circ(270, 118, 40), C["skyp"], 0.35, edge=0)
        pg.wash(rect(230, 120, 80, 38), C["umberl"], 0.7, edge=0)
        pg.ink(P([(230, 120), (310, 120)]), 0.7)
        pg.ink(crenel(230, 310, 120, 3, 2.6) and P(crenel(230, 310, 120, 3.2, 2.6)), 0.5)
        pg.wash(f"M262 158v-14a8 8 0 0 1 16 0v14Z", C["payne"], 0.85, edge=0)
        pg.ink(f"M262 158v-14a8 8 0 0 1 16 0v14", 0.7)
        pg.hatch([(232, 124), (308, 124), (308, 158), (232, 158)], 0, 4.2, 0.25, INK2, 0.6, rng=False)
        body = rect(254, 102, 32, 15)
        pg.wash(body, C["crimson"], 0.75)
        pg.ink(body + "".join(P([(x, 102), (x, 117)]) for x in (260, 266, 274, 280)), 0.5)
        for (y, w) in ((102, 26), (88, 20)):
            roof = [(270 - w - 4, y + 1), (270 - w, y - 3), (270 + w, y - 3), (270 + w + 4, y + 1)]
            pg.wash(P(roof + [(270 + w, y - 1), (270 - w, y - 1)], True), C["payne"], 0.85, edge=0)
            pg.ink(S([roof[0], (270 - w + 2, y - 2), (270, y - 3.5), (270 + w - 2, y - 2), roof[3]]), 0.6)
            pg.ink(P([(270 - w, y - 3), (270 + w, y - 3)]), 0.6)
        pg.wash(rect(258, 89, 24, 10), C["crimson"], 0.75)
        pg.ink(rect(258, 89, 24, 10), 0.5)
        pg.ink(P([(270, 81), (270, 77)]), 0.6)
    pg.leader(O(W_ / 2, 0), (238, 136), bend=0.12)
    return pg.svg()


# ------------------------------------------------------------------ Buddhist temples
def _pagoda(pg: Page, x, gy, h, n=7):
    """A multi-storey pagoda: storeys narrowing upwards, eaves with upturned corners, a spire."""
    base_w = h * 0.26
    sh = h * 0.8 / n
    y = gy
    for i in range(n):
        w = base_w * (1 - i * 0.075)
        body = rect(x - w / 2, y - sh * 0.62, w, sh * 0.62)
        pg.wash(body, C["sienna"] if i % 2 == 0 else C["bsienna"], 0.55, edge=0)
        pg.ink(P([(x - w / 2, y), (x - w / 2, y - sh * 0.62)]) + P([(x + w / 2, y), (x + w / 2, y - sh * 0.62)]), 0.5)
        pg.ink(rect(x - w * 0.12, y - sh * 0.5, w * 0.24, sh * 0.36), 0.35)
        ey = y - sh * 0.62
        ew = w / 2 + sh * 0.55
        eave = [(x - ew - 1.4, ey - 1.6), (x - ew + 1, ey), (x + ew - 1, ey), (x + ew + 1.4, ey - 1.6),
                (x + w / 2 + 0.5, ey - sh * 0.38), (x - w / 2 - 0.5, ey - sh * 0.38)]
        pg.wash(P(eave, True), C["payne"], 0.75, edge=0)
        pg.ink(S([eave[0], eave[1], (x, ey + 0.5), eave[2], eave[3]], False, 0.3) + P([eave[0], eave[5], eave[4], eave[3]]), 0.5)
        y = ey - sh * 0.38
    pg.ink(P([(x, y), (x, y - h * 0.16)]), 0.8)
    pg.ink("".join(ell(x, y - h * 0.03 - k * 2.4, 1.6, 0.6) for k in range(4)), 0.45)
    return y


def _hall(pg: Page, x, gy, w, h):
    """A temple hall: platform, red columns, a hip-and-gable roof with upturned corners."""
    pg.wash(rect(x - w / 2 - 3, gy - 3, w + 6, 3), C["greyl"], 0.9)
    pg.ink(rect(x - w / 2 - 3, gy - 3, w + 6, 3), 0.5)
    ch = h * 0.45
    pg.wash(rect(x - w / 2, gy - 3 - ch, w, ch), C["crimson"], 0.7)
    pg.ink("".join(P([(xx, gy - 3), (xx, gy - 3 - ch)]) for xx in [x - w / 2 + k * w / 5 for k in range(6)]), 0.5)
    ry = gy - 3 - ch
    roof = [(x - w / 2 - 10, ry - 2), (x - w / 2 - 6, ry + 1), (x + w / 2 + 6, ry + 1), (x + w / 2 + 10, ry - 2),
            (x + w * 0.3, ry - h * 0.45), (x - w * 0.3, ry - h * 0.45)]
    pg.wash(P(roof, True), C["payne"], 0.8)
    pg.ink(S([roof[0], roof[1], (x, ry + 2), roof[2], roof[3]], False, 0.3) + P([roof[0], roof[5], roof[4], roof[3]]), 0.6)
    # the main ridge, sitting on the roof, with its end ornaments
    pg.ink(P([roof[5], roof[4]]), 1.3)
    pg.ink(P([roof[5], (roof[5][0] - 1.5, roof[5][1] - 2.5)]) + P([roof[4], (roof[4][0] + 1.5, roof[4][1] - 2.5)]), 0.8)
    pg.hatch([roof[1], roof[2], roof[4], roof[5]], 90, 3.6, 0.25, INK2, 0.8, rng=False, inset=0.8)


def temples():
    pg = Page(21)
    gy = 140
    # misty mountains, a pine, the pagoda and a hall on a terrace
    for k, (pts, op) in enumerate((([(4, 92), (40, 54), (70, 66), (100, 38), (140, 70), (180, 46), (212, 72)], 0.35),
                                   ([(4, 112), (50, 84), (90, 98), (130, 78), (176, 96), (212, 88)], 0.45))):
        pg.wash(S(pts + [(212, 120), (4, 120)], True), C["sky"] if k == 0 else C["sapl"], op, edge=0)
        pg.ink(S(pts), 0.45, INK2, 0.7)
    pg.vignette(6, gy - 4, 210, 168, C["sapl"], 0.5, 3, seed=1)
    pg.ink(P([(10, gy), (206, gy)]), 0.6)
    _pagoda(pg, 60, gy, 104)
    _hall(pg, 140, gy, 56, 34)
    # pine: layered horizontal foliage pads on a leaning trunk
    pg.ink(S([(188, gy), (186, 120), (192, 96), (190, 80)]), 1.4, C["umber"])
    for x, y, w in ((178, 104, 14), (198, 96, 12), (184, 84, 12), (196, 116, 10)):
        pad = [(x - w, y), (x - w * 0.5, y - 4), (x + w * 0.5, y - 4.5), (x + w, y), (x, y + 1.5)]
        pg.wash(S(pad, True), C["hooker"], 0.75)
        pg.ink(scallop(pad[:4], 0.3), 0.45)
    # inset: a bracket set (dougong) under the eaves: blocks and arms stacked to carry the roof out
    with pg.inset_circle(270, 52, 34):
        pg.wash(circ(270, 52, 34), C["skyp"], 0.35, edge=0)
        pg.wash(rect(262, 70, 16, 18), C["crimson"], 0.75)
        pg.ink(rect(262, 70, 16, 18), 0.6)                    # column top
        d = ""
        tiers = [(64, 8), (56, 16), (48, 24), (40, 30)]
        for y, half in tiers:
            pg.wash(rect(270 - half, y, 2 * half, 4), C["sap"] if half % 16 == 0 else C["cerul"], 0.75, edge=0)
            d += rect(270 - half, y, 2 * half, 4)
            for sx in (-1, 1):
                d += rect(270 + sx * half - (5 if sx > 0 else 0) + (0 if sx > 0 else 0), y + 4, 5, 3.4)
        d += rect(264, 66, 12, 4)
        pg.ink(d, 0.5)
        pg.wash(P([(226, 40), (314, 40), (314, 30), (226, 24)], True), C["payne"], 0.75, edge=0)
        pg.ink(P([(226, 24), (314, 30)]) + P([(226, 40), (314, 40)]), 0.6)
    pg.leader((140, gy - 21), (240, 66), bend=-0.08)
    # where the temples are: dots clustering in the east of China
    m = Equi(100, 123, 21, 40, 240, 104, 66)
    with pg.inset_box(240, 104, 66, m.h):
        pg.wash(rect(240, 104, 66, m.h), C["sky"], 0.4, edge=0)
        pg.land(m.land(tol=1.0), C["sand"], 0.9)
        rng = random.Random(21)
        pts = []
        for lon, lat, pop, _ in cities():
            if 100 <= lon <= 123 and 21 <= lat <= 40 and pop > 800_000:
                for _ in range(3 + int(pop / 2e6)):
                    pts.append(m(lon + rng.gauss(0, 0.9), lat + rng.gauss(0, 0.7)))
        pg.dots(pts, 0.7, C["crimson"])
    return pg.svg()


# ------------------------------------------------------------------ castles
def castles():
    pg = Page(22)
    gy = 150
    # the rock it stands on
    rock = [(20, gy), (34, 128), (60, 118), (110, 114), (160, 118), (186, 128), (200, gy)]
    pg.wash(S(rock + [(200, 166), (20, 166)], True, 0.25), C["umberl"], 0.7)
    pg.ink(S(rock, False, 0.25), 0.7)
    pg.hatch([(36, 130), (60, 122), (70, 150), (34, 150)], -60, 2.2, 0.35, INK2, 0.7)
    pg.hatch([(160, 122), (186, 130), (196, 150), (166, 150)], -60, 2.2, 0.35, INK2, 0.7)
    pg.ground(10, 210, gy + 14, C["sapl"], 5, 0.5)
    # curtain wall with corner towers, a gatehouse, and the keep rising behind
    wall_y, wall_t = 120, 92
    pg.wash(rect(46, wall_t, 128, wall_y - wall_t), C["greyl"], 0.85)
    pg.ink(P([(46, wall_y), (46, wall_t)]) + P(crenel(46, 174, wall_t, 3.4, 3)) + P([(174, wall_t), (174, wall_y)]), 0.6)
    # keep (square great tower) behind the curtain
    kx0, kx1, kt = 90, 130, 40
    pg.wash(rect(kx0, kt, kx1 - kx0, wall_t - kt), C["greyl"], 0.9)
    pg.wash(rect(kx0 + 26, kt, 14, wall_t - kt), C["slate"], 0.4, edge=0)
    pg.ink(P([(kx0, wall_t - 3), (kx0, kt)]) + P(crenel(kx0, kx1, kt, 3.4, 3.4)) + P([(kx1, kt), (kx1, wall_t - 3)]), 0.7)
    pg.ink(rect(108, 54, 4, 8) + rect(98, 70, 3, 7) + rect(120, 70, 3, 7), 0.5)
    pg.ink(P([(119, kt - 3.4), (119, kt - 16)]), 0.5)
    pg.wash(P([(119, kt - 16), (130, kt - 13), (119, kt - 10)], True), C["red"], 0.85, edge=0)
    # round corner towers, taller than the wall, conical tops left off (battlemented)
    for tx in (46, 174):
        tt = wall_t - 16
        pg.wash(rect(tx - 9, tt, 18, wall_y - tt + 2), C["greyl"], 0.9)
        pg.wash(rect(tx + 3, tt, 6, wall_y - tt + 2), C["slate"], 0.4, edge=0)
        pg.ink(P([(tx - 9, wall_y + 2), (tx - 9, tt)]) + P(crenel(tx - 9, tx + 9, tt, 3, 3)) + P([(tx + 9, tt), (tx + 9, wall_y + 2)]),
               0.7)
        pg.ink(rect(tx - 1, tt + 10, 2, 6) + rect(tx - 1, tt + 26, 2, 6), 0.5)
    # gatehouse: two half-round towers flanking an arched gate with its portcullis
    gx = 110
    for sx in (-1, 1):
        x = gx + sx * 12
        pg.wash(rect(x - 6, wall_t - 8, 12, wall_y - wall_t + 9), C["greyl"], 0.95)
        pg.ink(P([(x - 6, wall_y + 1), (x - 6, wall_t - 8)]) + P(crenel(x - 6, x + 6, wall_t - 8, 3, 2.6)) +
               P([(x + 6, wall_t - 8), (x + 6, wall_y + 1)]), 0.65)
    pg.wash(f"M{gx - 6} {wall_y}v-12a6 6 0 0 1 12 0v12Z", C["payne"], 0.85, edge=0)
    pg.ink(f"M{gx - 6} {wall_y}v-12a6 6 0 0 1 12 0v12" + "".join(P([(x, wall_y - 17), (x, wall_y - 6)]) for x in (gx - 3, gx, gx + 3)) +
           P([(gx - 5.6, wall_y - 10), (gx + 5.6, wall_y - 10)]), 0.55)
    pg.ink(S([(gx, wall_y + 1), (gx - 10, 136), (gx + 6, 146), (gx - 4, gy + 10)]), 0.9, C["umberl"])    # path up
    pg.ink("".join(P([(x, y), (x + 8, y)]) for x, y in ((56, 100), (66, 108), (150, 104), (140, 112))), 0.3, INK2)
    pg.person(150, gy + 4, 4)
    # inset: battlement close-up (merlon with an arrow slit, crenel) with a defender for scale
    with pg.inset_circle(268, 52, 34):
        pg.wash(circ(268, 52, 34), C["skyp"], 0.35, edge=0)
        top = 40
        pg.wash(P(crenel(234, 304, top, 14, 14) + [(304, 92), (234, 92)], True), C["greyl"], 0.9)
        pg.ink(P(crenel(234, 304, top, 14, 14)), 0.75)
        for mx in (241, 269, 297):
            pg.ink(rect(mx - 1, top - 10, 2, 9), 0.6)
        pg.ink("".join(P([(234, y), (304, y)]) for y in (48, 58, 68, 78)) +
               "".join(P([(x + (6 if (y // 10) % 2 else 0), y), (x + (6 if (y // 10) % 2 else 0), y + 10)])
                       for y in (48, 58, 68) for x in range(238, 300, 12)), 0.3, INK2)
        pg.person(259, top, 15)
    pg.leader((60, wall_t + 2), (236, 64), bend=-0.08)
    # the same query can map lighthouses
    with pg.inset_circle(268, 136, 30):
        pg.wash(circ(268, 136, 30), C["skyp"], 0.4, edge=0)
        pg.wash(rect(238, 152, 60, 16), C["cerul"], 0.6, edge=0)
        tw = [(262, 154), (264, 122), (272, 122), (274, 154)]
        pg.wash(P(tw, True), C["white"], 0.95, edge=0.5)
        pg.wash(P([(263.3, 136), (272.7, 136), (273.3, 144), (262.7, 144)], True), C["red"], 0.85, edge=0)
        pg.wash(P([(264.2, 124), (271.8, 124), (272.2, 130), (263.8, 130)], True), C["red"], 0.85, edge=0)
        pg.ink(P(tw, True), 0.6)
        pg.wash(rect(264, 114, 8, 8), C["lemon"], 0.9)
        pg.ink(rect(264, 114, 8, 8) + P([(262, 114), (268, 108), (274, 114)]) + P([(262, 122), (274, 122)]), 0.55)
        for a in (-0.2, 0.2):
            pg.ink(P([(274, 118 + a * 10), (296, 112 + a * 40)]), 0.4, C["ochre"])
            pg.ink(P([(262, 118 + a * 10), (240, 112 + a * 40)]), 0.4, C["ochre"])
        pg.ink(S([(240, 156), (250, 152), (258, 154)]) + S([(276, 154), (290, 151), (300, 154)]), 0.6, C["umber"])
    return pg.svg()
