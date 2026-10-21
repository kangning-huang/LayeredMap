"""Climate: temperature anomalies, sea surface temperature, precipitation."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, Equi, Ortho, P, S, circ, ell, ell_pts, rect,
                  scallop, lerp, lerp2, mix)


def _globe(pg: Page, o: Ortho, land_color=None, sea=None, grat=30):
    cx, cy, r = o.cx, o.cy, o.r
    pg.wash(circ(cx, cy, r), sea or C["skyp"], 0.7, edge=0)
    land = o.land(tol=0.9)
    pg.land(land, land_color or C["sand"], 0.8)
    # graticule in pencil
    d = ""
    for lat in range(-60, 90, grat):
        for run in o.line([(lo, lat) for lo in range(-180, 181, 6)]):
            d += P(run)
    for lon in range(-180, 180, grat):
        for run in o.line([(lon, la) for la in range(-90, 91, 6)]):
            d += P(run)
    pg.ink(d, 0.3, INK2, 0.5)
    return land


def _anomaly(year):
    """A smooth stand-in for the global annual anomaly against 1951-1980 (deg C)."""
    t = year
    base = -0.25 - 0.12 * math.exp(-((t - 1910) / 14) ** 2) + 0.28 / (1 + math.exp(-(t - 1938) / 6))
    base += 1.05 / (1 + math.exp(-(t - 2005) / 16)) - 0.04
    return base


def temperature():
    pg = Page(4)
    o = Ortho(15, 35, 86, 94, 66)
    with pg.inset_circle(o.cx, o.cy, o.r, ring=0.9):
        _globe(pg, o)
        # anomaly washes: strongest warming in the Arctic and over land, a cool patch south of Greenland
        for lon, lat, rx, ry, c, op in ((20, 78, 60, 16, C["red"], 0.55), (60, 60, 40, 12, C["red"], 0.45),
                                       (-100, 62, 40, 14, C["orange"], 0.5), (15, 48, 24, 10, C["orange"], 0.5),
                                       (30, 20, 30, 10, C["ochre"], 0.45), (100, 40, 26, 10, C["orange"], 0.45),
                                       (-40, 52, 12, 6, C["cerul"], 0.65), (-25, 5, 22, 7, C["ochrel"], 0.4)):
            x, y, v = o(lon, lat)
            if v:
                pg.wash(S(ell_pts(x, y, rx * 0.5, ry * 0.6, 12), True), c, op, edge=0)
    pg.ink(S(ell_pts(o.cx, o.cy, o.r + 0.6, o.r + 0.6, 30, 3.6, 5.6)), 0.5, INK2)     # limb shading line
    # a thermometer study beside the globe
    tx, top, bot = 176, 40, 146
    pg.wash(P([(tx - 3, top), (tx + 3, top), (tx + 3, bot - 4), (tx - 3, bot - 4)], True), C["skyp"], 0.6, edge=0)
    pg.wash(circ(tx, bot, 6.5), C["red"], 0.85)
    pg.wash(rect(tx - 1.3, 82, 2.6, bot - 86), C["red"], 0.85, edge=0)
    pg.ink(f"M{tx - 3} {bot - 5}V{top}a3 3 0 0 1 6 0V{bot - 5}", 0.8)
    pg.ink(S(ell_pts(tx, bot, 6.5, 6.5, 20, -1.1, 4.25)), 0.8)
    pg.ink("".join(P([(tx + 3, y), (tx + (6 if i % 5 == 0 else 4.5), y)]) for i, y in
                   enumerate(range(int(top + 6), int(bot - 8), 4))), 0.4)
    pg.raw(f'<path d="M{tx - 1.6} {bot - 3}a2 2 0 0 1 2-2" stroke="{C["white"]}" stroke-width=".8" fill="none"/>')
    pg.arrow([(188, 104), (188, 84)], RED_PENCIL, 1.0, 3)
    # a hand-drawn chart: annual anomalies against the 1951-1980 average, then the warming stripes
    x0, y0, w, h = 214, 20, 98, 70
    with pg.inset_box(x0, y0, w, h):
        zero = y0 + h * 0.62
        bx0, bx1 = x0 + 6, x0 + w - 4
        yrs = list(range(1880, 2025, 3))
        bw = (bx1 - bx0) / len(yrs)
        rng = random.Random(8)
        # the 1951-1980 base period as a pale band
        b0 = bx0 + (1951 - 1880) / 3 * bw
        b1 = bx0 + (1980 - 1880) / 3 * bw
        pg.wash(rect(b0, y0 + 4, b1 - b0, h - 8), C["ochrel"], 0.35, edge=0)
        for i, yr in enumerate(yrs):
            a = _anomaly(yr) + rng.uniform(-0.08, 0.08)
            hh = a * 34
            c = C["red"] if a > 0 else C["cerul"]
            x = bx0 + i * bw
            pg.wash(rect(x + .25, min(zero, zero - hh), bw - .5, abs(hh)), c, 0.85, edge=0)
        pg.ink(P([(bx0 - 2, zero), (bx1 + 1, zero)]), 0.5)
        pg.ink(P([(bx0 - 2, y0 + 4), (bx0 - 2, y0 + h - 4)]) +
               "".join(P([(bx0 - 3.5, zero - k * 10), (bx0 - 2, zero - k * 10)]) for k in (-1, 1, 2, 3)), 0.45)
    sy = y0 + h + 8
    with pg.inset_box(x0, sy, w, 16):
        n_ = 36
        for i in range(n_):
            yr = 1880 + i * 4
            a = _anomaly(yr)
            t = max(-1, min(1, a / 0.9))
            c = mix(C["white"], C["red"], t) if t > 0 else mix(C["white"], C["ultra"], -t * 1.6 if -t * 1.6 < 1 else 1)
            pg.raw(f'<rect x="{x0 + i * w / n_:.1f}" y="{sy}" width="{w / n_ + .3:.1f}" height="16" fill="{c}"/>')
    pg.leader((140, 60), (214, 54), dot_a=True)
    return pg.svg()


# ------------------------------------------------------------------ sea surface temperature
def _ship(pg: Page, x, y, s=1.0):
    """A three-masted sailing ship, hull length about 34 s, waterline at y."""
    def T(pts):
        return [(x + px * s, y + py * s) for px, py in pts]
    hull = T([(-17, -4), (17, -5), (20, -8), (14, 0), (-14, 0), (-18, -6)])
    pg.wash(P(hull, True), C["umber"], 0.85)
    pg.ink(P(hull, True), 0.6)
    for mx, mh in ((-9, 24), (1, 28), (10, 22)):
        pg.ink(P(T([(mx, -4.5), (mx, -4.5 - mh)])), 0.55)
        for k, (yy, ww) in enumerate(((-9, 6), (-15, 5), (-20.5, 4))):
            if -yy < mh:
                sail = T([(mx - ww, yy + 1 - 4.5 + 2), (mx + ww, yy + 1 - 4.5 + 2), (mx + ww * .85, yy - 4.5 - 2.5),
                          (mx - ww * .85, yy - 4.5 - 2.5)])
                pg.wash(P(sail, True), C["white"], 0.9, edge=0.6)
                pg.ink(S([sail[0], lerp2(sail[0], sail[1], .5), sail[1]]) + P([sail[1], sail[2], sail[3], sail[0]]), 0.4)
    pg.ink(P(T([(17, -5), (27, -14)])), 0.45)                    # bowsprit


def _argo(pg: Page, x, y, s=1.0):
    """A profiling float: a vertical cylinder with a collar and an antenna."""
    body = [(x - 1.4 * s, y - 4 * s), (x + 1.4 * s, y - 4 * s), (x + 1.4 * s, y + 6 * s), (x - 1.4 * s, y + 6 * s)]
    pg.wash(P(body, True), C["ochre"], 0.85)
    pg.ink(P(body, True) + P([(x - 2.4 * s, y - 1 * s), (x + 2.4 * s, y - 1 * s)]) + P([(x, y - 4 * s), (x, y - 8.5 * s)]),
           0.5)


def sea_temperature():
    pg = Page(5)
    hz = 62
    # sky and sea
    pg.vignette(6, 18, 212, hz, C["skyp"], 0.45, 4, seed=1)
    gid = pg.lingrad([(0, C["ochrel"], 0.6), (0.22, C["sky"], 0.5), (0.8, C["ultra"], 0.55), (1, C["ultra"], 0.2)])
    sea = [(6, hz), (212, hz), (214, 120), (208, 172), (8, 170), (4, 120)]
    pg.wash_grad(S(sea, True, 0.2), gid)
    pg.ink(P([(6, hz), (212, hz)]), 0.7)
    # little wave ticks on the surface
    pg.ink("".join(f"M{x} {hz + 3 + (x % 7) * .3:.1f}q2 -1.4 4 0" for x in range(14, 206, 13)), 0.35, INK2)
    _ship(pg, 46, hz, 1.25)
    # the thermocline: warm mixed layer over cold deep water
    pg.ink(S([(6, 84), (60, 82), (120, 86), (212, 83)]), 0.4, INK2, 0.7, "3 2")
    # an Argo float's ten-day cycle: sink, drift at depth, sink deeper, rise measuring, surface and report
    path_down = [(104, hz + 2), (106, 100), (108, 128)]
    drift = [(108, 128), (130, 130), (150, 127), (164, 129)]
    deep = [(164, 129), (166, 158)]
    up = [(166, 158), (178, 120), (186, 90), (190, hz + 2)]
    pg.arrow(path_down, BLUE_ARROW, 0.9, 3)
    pg.ink(S(drift), 0.7, BLUE_ARROW, 0.9, "2 1.6")
    pg.arrow(deep, BLUE_ARROW, 0.9, 3, smooth=False)
    pg.arrow(up, RED_PENCIL, 0.9, 3)
    for t in (0.15, 0.32, 0.5, 0.68, 0.85):           # measurements taken on the way up
        i = min(int(t * 3), 2)
        u = t * 3 - i
        x, y = lerp2(up[i], up[i + 1], u)
        pg.dot(x, y, 0.9, INK)
    _argo(pg, 190, hz - 4, 1.0)
    _argo(pg, 136, 127, 0.8)
    pg.ink(P([(190, hz - 13), (198, hz - 22)]) + P([(193, hz - 18), (199, hz - 19)]), 0.35, INK2, 0.8, "1 1.2")
    # inset: a bucket hauled on deck, a thermometer in it (how the old ship records were made)
    with pg.inset_circle(266, 44, 30):
        pg.wash(circ(266, 44, 30), C["skyp"], 0.35, edge=0)
        bucket = [(254, 38), (278, 38), (275, 66), (257, 66)]
        pg.wash(P(bucket, True), C["umberl"], 0.8)
        pg.wash(S(ell_pts(266, 38, 12, 3, 16), True), C["cerul"], 0.8, edge=0)
        pg.ink(P(bucket[1:] + [bucket[0]]) + S(ell_pts(266, 38, 12, 3, 20), True), 0.7)
        pg.ink(S([(254, 38), (256, 22), (266, 18), (276, 22), (278, 38)]), 0.5)        # rope handle
        pg.ink(S([(266, 18), (268, 8), (264, 0)]), 0.5)
        pg.ink(P([(258, 46), (276, 46)]) + P([(258, 56), (276, 56)]), 0.35, INK2)      # canvas seams
        pg.wash(P([(269, 20), (272, 20), (272, 52), (269, 52)], True), C["white"], 0.9)
        pg.ink(P([(269, 20), (272, 20), (272, 52), (269, 52)], True), 0.5)
        pg.raw(f'<path d="M269.9 36h1.2v14h-1.2z" fill="{C["red"]}"/>')
    pg.leader((52, hz - 6), (238, 52), bend=-0.08)
    # inset: the tropical Pacific in an El Nino year, warm water spreading east along the equator
    m = Equi(120, 290, -28, 28, 230, 102, 82)
    with pg.inset_box(230, 102, 82, m.h):
        pg.wash(rect(230, 102, 82, m.h), C["sky"], 0.55, edge=0)
        tongue = [m(130, 8), m(180, 7), m(230, 6), m(270, 3), m(280, -4), m(250, -6), m(200, -7), m(140, -9)]
        pg.wash(S(tongue, True), C["orange"], 0.6)
        pg.wash(S([m(220, 3), m(262, 2), m(272, -2), m(250, -4), m(222, -3)], True), C["red"], 0.6, edge=0)
        land = m.land(tol=1.2)
        pg.land(land, C["sand"], 0.9)
        pg.ink(P([m(120, 0), m(290, 0)]), 0.35, INK2, 0.7, "2 1.5")
        pg.arrow([m(160, -14), m(200, -15), m(250, -13)], RED_PENCIL, 0.8, 2.6)
    return pg.svg()


# ------------------------------------------------------------------ precipitation (monsoon rain)
def precipitation():
    pg = Page(6)
    gy = 132
    # monsoon clouds and rain over terraced fields
    cloud = [(10, 60), (14, 44), (30, 32), (52, 22), (80, 18), (110, 20), (138, 16), (166, 24), (190, 34), (204, 50),
             (200, 62)]
    pg.wash(S(cloud + [(150, 66), (100, 64), (50, 66)], True), C["slate"], 0.55)
    pg.wash(S([(14, 60), (60, 54), (110, 58), (160, 52), (200, 60), (150, 66), (100, 64), (50, 66)], True), C["payne"],
            0.5, edge=0)
    pg.ink(scallop(cloud, 0.28), 0.75)
    pg.ink(P([(10, 62), (202, 62)]), 0.5, INK2, 0.6)
    rng = random.Random(6)
    for x0, x1 in ((24, 84), (110, 180)):
        gid = pg.lingrad([(0, C["payne"], 0.55), (1, C["slate"], 0.15)])
        pg.wash_grad(P([(x0, 62), (x1, 62), (x1 - 10, gy), (x0 - 10, gy)], True), gid)
        pg.ink("".join(P([(x, 64 + rng.uniform(0, 5)), (x - 8, gy - 4 - rng.uniform(0, 14))])
                       for x in range(x0 + 2, x1, 4)), 0.3, INK2, 0.7)
    # hills with terraced paddies, water in the terraces
    hill = [(4, 112), (30, 96), (60, 90), (92, 100), (124, 92), (160, 98), (196, 90), (212, 96), (212, 150), (4, 150)]
    pg.wash(S(hill, True, 0.3), C["sapl"], 0.65, edge=0)
    pg.ink(S(hill[:8], False, 0.3), 0.7)
    for k in range(5):
        y = 106 + k * 9
        pts = [(6, y + 4 - k * .5), (40, y - 2), (80, y + 3), (120, y - 1), (160, y + 3), (210, y - 1)]
        pg.ink(S(pts), 0.45, INK2)
        pg.wash(S(pts + [(p[0], p[1] + 2.4) for p in reversed(pts)], True), C["cerul"], 0.45, edge=0)
    pg.ink("".join(f"M{x} {y}l.8-2.6M{x + 2} {y}l-.4-2.8" for x, y in
                   [(26, 118), (52, 113), (78, 120), (104, 115), (134, 119), (162, 113), (188, 118),
                    (40, 136), (70, 140), (98, 136), (128, 140), (156, 136), (182, 140)]), 0.4, C["hooker"])
    # a rain gauge standing in the field
    gx = 150
    pg.wash(P([(gx - 3, 128), (gx + 3, 128), (gx + 3, 146), (gx - 3, 146)], True), C["greyl"], 0.9)
    pg.ink(P([(gx - 3, 128), (gx + 3, 128), (gx + 3, 146), (gx - 3, 146)], True) + P([(gx, 146), (gx, 152)]), 0.6)
    pg.leader((gx + 3, 132), (234, 128), bend=-0.05)
    with pg.inset_circle(266, 136, 32):
        pg.wash(circ(266, 136, 32), C["skyp"], 0.35, edge=0)
        # outer can, funnel, inner measuring tube with water and graduations
        pg.ink(P([(252, 112), (252, 168)]) + P([(280, 112), (280, 168)]), 0.8)
        pg.ink(P([(252, 112), (280, 112)]), 0.5)
        pg.ink(P([(252, 114), (264, 126), (264, 134)]) + P([(280, 114), (268, 126), (268, 134)]), 0.7)
        pg.wash(P([(262, 150), (270, 150), (270, 168), (262, 168)], True), C["cerul"], 0.8)
        pg.ink(P([(262, 132), (262, 170)]) + P([(270, 132), (270, 170)]), 0.6)
        pg.ink("".join(P([(270, y), (273 if i % 2 else 275, y)]) for i, y in enumerate(range(136, 168, 4))), 0.4)
        for x in (258, 266, 274):
            pg.ink(P([(x, 104), (x - 2, 110)]), 0.6, BLUE_ARROW)
    # the summer monsoon: moist south-westerlies from the Indian Ocean onto South and South-East Asia
    m = Equi(55, 130, -8, 42, 230, 14, 82)
    with pg.inset_box(230, 14, 82, m.h):
        pg.wash(rect(230, 14, 82, m.h), C["sky"], 0.5, edge=0)
        pg.land(m.land(tol=1.2), C["sand"], 0.9)
        pg.wash(S([m(74, 24), m(80, 30), m(90, 27), m(98, 20), m(104, 14), m(96, 10), m(84, 12), m(76, 16)], True),
                C["hooker"], 0.55, edge=0)
        for a, b in (((60, 0), (74, 16)), ((70, -4), (86, 16)), ((84, -4), (96, 16)), ((96, 2), (106, 18))):
            pa, pb = m(*a), m(*b)
            pg.arrow([pa, lerp2(pa, pb, .5), pb], BLUE_ARROW, 0.9, 2.6)
    return pg.svg()
