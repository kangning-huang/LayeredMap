"""People: cities, life expectancy, population."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, Equi, P, S, circ, ell, ell_pts, rect, scallop,
                  lerp, lerp2, mix, cities, countries, continent, country_rings, inside)


# ------------------------------------------------------------------ cities
def cities_art():
    pg = Page(11)
    rng = random.Random(11)
    # hills behind, a river through the city, towers at the centre thinning out to low houses
    hills = [(6, 70), (40, 58), (80, 64), (120, 52), (160, 60), (208, 54)]
    pg.wash(S(hills + [(208, 80), (6, 80)], True), C["sky"], 0.45, edge=0)
    pg.ink(S(hills), 0.55, INK2)
    gy = 120
    pg.vignette(6, gy - 4, 210, 168, C["sapl"], 0.4, 4, seed=1)
    pg.wash(S([(6, 150), (60, 140), (110, 146), (160, 136), (210, 142), (210, 152), (160, 146), (110, 156), (60, 150),
               (6, 160)], True), C["cerul"], 0.6, edge=0)
    pg.ink(S([(6, 150), (60, 140), (110, 146), (160, 136), (210, 142)]), 0.45, INK2)
    blds = []
    x = 10
    while x < 204:
        d = abs(x - 104) / 96
        h = (8 + 70 * (1 - d) ** 2.2) * rng.uniform(0.6, 1.1) if d < 0.95 else rng.uniform(5, 9)
        w = rng.uniform(5, 9) if d < 0.5 else rng.uniform(6, 11)
        blds.append((x, w, h))
        x += w + rng.uniform(0.2, 1.8)
    for x, w, h in sorted(blds, key=lambda b: b[2]):
        top = gy - h
        c = rng.choice([C["greyl"], C["ochrel"], C["sky"], C["umberl"]])
        pg.wash(rect(x, top, w, h), c, 0.65, edge=0)
        pg.wash(rect(x + w * 0.6, top, w * 0.4, h), C["slate"], 0.3, edge=0)
    d = ""
    for x, w, h in blds:
        d += P([(x, gy), (x, gy - h), (x + w, gy - h), (x + w, gy)])
    pg.ink(d, 0.5)
    # window dots on the tall ones
    wd = ""
    for x, w, h in blds:
        if h > 30:
            for yy in range(int(gy - h + 4), gy - 3, 5):
                wd += f"M{x + 2:.1f} {yy}h{w - 4:.1f}"
    pg.ink(wd, 0.25, INK2, 0.7, "0.8 1.2")
    pg.ink(P([(6, gy), (210, gy)]), 0.6)
    # inset: a map with circles sized by population (the bundled cities, East Asia)
    m = Equi(98, 146, 18, 46, 228, 22, 84)
    with pg.inset_box(228, 22, 84, m.h):
        pg.wash(rect(228, 22, 84, m.h), C["sky"], 0.4, edge=0)
        pg.land(m.land(tol=1.2), C["sand"], 0.85)
        pts = []
        for lon, lat, pop, cap in cities():
            if 98 <= lon <= 146 and 18 <= lat <= 46 and pop > 1_500_000:
                x, y = m(lon, lat)
                pts.append((x, y, 0.5 + math.sqrt(pop / 1e6) * 1.15, cap))
        pts.sort(key=lambda p: -p[2])
        for x, y, r, cap in pts:
            pg.wash(circ(x, y, r), C["red"], 0.45, edge=0.6)
        pg.ink("".join(circ(x, y, r) for x, y, r, cap in pts if r > 2), 0.35)
        for x, y, r, cap in pts:
            if cap:
                pg.dot(x, y, 0.8)
    pg.leader((104, gy - 60), (228, 50), bend=-0.06)
    # a size key: three circles, small to large
    for i, r in enumerate((1.6, 3.2, 5.5)):
        cx = 244 + i * 22
        pg.wash(circ(cx, 150, r), C["red"], 0.45, edge=0.6)
        pg.ink(circ(cx, 150, r), 0.45)
    pg.arrow([(236, 162), (300, 162)], INK, 0.6, 2.2, smooth=False)
    return pg.svg()


# ------------------------------------------------------------------ life expectancy
def _standing(pg: Page, x, gy, h, col, lean=0.0, cane=False, arm_out=0.0):
    """A standing figure, feet at gy, height h, coat colour col."""
    s = h / 24
    neck = (x + lean * .5, gy - h + 5.2 * s)
    hip = (x, gy - h * 0.44)
    pg.wash(P([(neck[0] - 2.6 * s, neck[1] + .4), (neck[0] + 2.6 * s, neck[1] + .4), (hip[0] + 2.5 * s, hip[1] + 1),
               (hip[0] - 2.5 * s, hip[1] + 1)], True), col, 0.75, edge=0)
    pg.wash(circ(neck[0], gy - h + 2.6 * s, 2.5 * s), C["sand"], 0.8, edge=0)
    hand = (x + 3.8 * s + lean + arm_out, gy - h * 0.48 - arm_out * 0.4)
    pg.ink(circ(neck[0], gy - h + 2.6 * s, 2.5 * s) + P([neck, hip]) + P([hip, (x - 2.6 * s, gy)]) + P([hip, (x + 2.6 * s, gy)]) +
           P([(neck[0], neck[1] + 1), (x - 3.4 * s, gy - h * 0.52)]) + P([(neck[0], neck[1] + 1), hand]), 0.8)
    if cane:
        pg.ink(P([hand, (hand[0] + 1.2 * s, gy)]), 0.8, C["umber"])
    return hand


def _seated(pg: Page, x, seat_y, gy, h, col, cane=False):
    """A figure seated on a bench facing right: hip at (x, seat_y), feet on the ground at gy."""
    s = h / 24
    hip = (x, seat_y - 1)
    neck = (x - 0.6 * s, seat_y - h * 0.42)
    knee = (x + 6.5 * s, seat_y - 1.4)
    foot = (x + 7 * s, gy)
    pg.wash(P([(neck[0] - 2.6 * s, neck[1] + .5), (neck[0] + 2.6 * s, neck[1] + .5), (hip[0] + 2.4 * s, hip[1]),
               (hip[0] - 2.4 * s, hip[1])], True), col, 0.75, edge=0)
    pg.wash(circ(neck[0] + 0.4 * s, neck[1] - 2.8 * s, 2.5 * s), C["sand"], 0.8, edge=0)
    hand = (x + 5 * s, seat_y - 4 * s)
    pg.ink(circ(neck[0] + 0.4 * s, neck[1] - 2.8 * s, 2.5 * s) + P([neck, hip]) + P([hip, knee, foot]) +
           P([(foot[0], gy), (foot[0] + 2.4 * s, gy)]) + P([(neck[0], neck[1] + 1.2), (x + 2.4 * s, seat_y - 5 * s), hand]), 0.8)
    if cane:
        pg.ink(P([(hand[0] + .5, hand[1]), (hand[0] + 3.5 * s, gy)]), 0.8, C["umber"])


def life_expectancy():
    """Four generations at a park bench (a grandmother seated with her cane, her son, a grandchild,
    a toddler with a ball); the data, a country-coded table joined to a world map, as small insets."""
    pg = Page(12)
    gy = 150
    pg.ground(6, 214, gy + 4, C["sapl"], 7, 0.7)
    pg.grass(14, 206, gy + 1, 16, 3.4, seed=4)
    # a shade tree behind, its branches inside the crown
    pg.ink(S([(176, gy), (175, 116), (178, 84)]) + S([(176, 104), (190, 88)]) + S([(177, 98), (164, 82)]), 2.0, C["umber"])
    crown = [(140, 72), (146, 46), (166, 30), (192, 30), (212, 48), (212, 74), (196, 90), (164, 92), (146, 86)]
    pg.wash(S(crown, True), C["sapl"], 0.8)
    pg.wash(S([(146, 80), (176, 72), (210, 74), (196, 90), (164, 92)], True), C["hooker"], 0.5, edge=0)
    pg.ink(scallop(crown, 0.18, True), 0.6, INK2)
    # the bench (seat about a quarter of an adult's height): slatted seat and back, iron ends
    sy = gy - 13
    bx0, bx1 = 30, 94
    pg.wash(P([(bx0, sy - 21), (bx1, sy - 21), (bx1, sy - 9), (bx0, sy - 9)], True), C["sienna"], 0.55, edge=0)
    pg.wash(P([(bx0 - 2, sy - 2), (bx1 + 2, sy - 2), (bx1 + 4, sy + 1.5), (bx0 - 4, sy + 1.5)], True), C["sienna"], 0.75, edge=0)
    pg.ink("".join(P([(bx0, y), (bx1, y)]) for y in (sy - 21, sy - 17, sy - 13, sy - 9)) +
           P([(bx0 - 2, sy - 2), (bx1 + 2, sy - 2)]) + P([(bx0 - 4, sy + 1.5), (bx1 + 4, sy + 1.5)]), 0.6)
    for ex in (bx0 + 2, bx1 - 2):
        pg.ink(S([(ex, sy - 23), (ex - 1, sy - 8), (ex + 1, sy + 1.5), (ex - 3, gy)]) + P([(ex + 1, sy + 1.5), (ex + 4, gy)]), 1.1)
    _seated(pg, 50, sy - 2, gy, 56, C["violet"], cane=True)
    _standing(pg, 112, gy, 58, C["ultra"])
    _standing(pg, 132, gy, 36, C["ochre"], arm_out=2)
    # toddler sitting on the grass with a ball
    tx, k = 150, 1.4
    pg.wash(S([(tx - 4 * k, gy), (tx - 3.6 * k, gy - 7 * k), (tx + 2.6 * k, gy - 7.4 * k), (tx + 4 * k, gy - k), (tx + 9 * k, gy - k),
               (tx + 9 * k, gy)], True), C["rose"], 0.8, edge=0)
    pg.wash(circ(tx - .4 * k, gy - 10.6 * k, 3.2 * k), C["sand"], 0.8, edge=0)
    pg.ink(circ(tx - .4 * k, gy - 10.6 * k, 3.2 * k) + S([(tx - 3.6 * k, gy - 7 * k), (tx - 4 * k, gy - 2 * k), (tx - 3 * k, gy)]) +
           S([(tx + 2.6 * k, gy - 7.4 * k), (tx + 3.6 * k, gy - 2 * k), (tx + 9 * k, gy - k)]) +
           P([(tx + 1.6 * k, gy - 6.4 * k), (tx + 7 * k, gy - 5 * k)]), 0.75)
    pg.wash(circ(tx + 11 * k, gy - 3.4, 3.4), C["red"], 0.85)
    pg.ink(circ(tx + 11 * k, gy - 3.4, 3.4), 0.6)
    # inset: how long a life lasts, 1900 against today (bars on a ruler)
    x0, x1, by = 232, 310, 22
    with pg.inset_box(226, 14, 90, 44):
        span = x1 - x0
        pg.ink(P([(x0, by + 26), (x1, by + 26)]) + "".join(P([(x0 + span * a / 80, by + 26), (x0 + span * a / 80, by + 23.5)])
                                                          for a in range(0, 81, 10)), 0.5)
        pg.wash(rect(x0, by, span * 32 / 80, 7), C["greyl"], 0.9)
        pg.ink(rect(x0, by, span * 32 / 80, 7), 0.5)
        pg.wash(rect(x0, by + 11, span * 73 / 80, 7), C["sap"], 0.85)
        pg.ink(rect(x0, by + 11, span * 73 / 80, 7), 0.5)
        pg.arrow([(x0 + span * 32 / 80 + 2, by + 4), (x0 + span * 52 / 80, by + 7), (x0 + span * 73 / 80 - 1, by + 12)],
                 RED_PENCIL, 0.8, 2.4)
    pg.leader((112, gy - 40), (226, 40), bend=-0.08)
    # inset: the table has only country codes, so it is joined to a world map
    m = Equi(-170, 180, -56, 76, 226, 74, 90)
    with pg.inset_box(226, 74, 90, m.h):
        pg.wash(rect(226, 74, 90, m.h), C["sky"], 0.35, edge=0)
        shade = {"Africa": 0.15, "Asia": 0.55, "South America": 0.6, "North America": 0.75, "Europe": 0.9,
                 "Oceania": 0.85}
        for cont, t in shade.items():
            isos = {i for i, _ in countries() if continent(i) == cont}
            pg.land(m.land(tol=1.0, only=isos), mix(C["ochrel"], C["hooker"], t), 0.9, 0.8)
    for i in range(5):
        pg.wash(rect(240 + i * 12, 156, 11, 5), mix(C["ochrel"], C["hooker"], i / 4), 0.9, edge=0)
    pg.ink(rect(240, 156, 60, 5), 0.45)
    pg.arrow([(240, 166), (300, 166)], INK, 0.6, 2.2, smooth=False)
    return pg.svg()


# ------------------------------------------------------------------ population (the Hu Line)
def _yurt(pg: Page, cx, cy, s):
    wall = [(cx - 12 * s, cy), (cx - 12 * s, cy - 7 * s), (cx + 12 * s, cy - 7 * s), (cx + 12 * s, cy)]
    pg.wash(P(wall, True), C["white"], 0.9, edge=0.5)
    pg.wash(P([(cx - 13 * s, cy - 7 * s), (cx, cy - 15 * s), (cx + 13 * s, cy - 7 * s)], True), C["greyl"], 0.85)
    pg.ink(P(wall) + P([(cx - 13 * s, cy - 7 * s), (cx, cy - 15 * s), (cx + 13 * s, cy - 7 * s)]) +
           rect(cx - 2.6 * s, cy - 6 * s, 5.2 * s, 6 * s), 0.6)
    pg.wash(rect(cx - 2.6 * s, cy - 6 * s, 5.2 * s, 6 * s), C["red"], 0.8, edge=0)
    pg.ink("".join(P([(cx + k * 3 * s, cy - 7 * s), (cx + k * 3 * s, cy)]) for k in (-3, -2, 2, 3)), 0.3, INK2)


def population():
    """A sketch-map of China with a loose watercolour wash for where people live: dense east and
    south-east of the Hu Line (Heihe to Tengchong), sparse to the north-west. The wash is
    impressionistic (centred on the great plains and basins), not gridded data."""
    pg = Page(13)
    m = Equi(73, 135, 17, 54, 6, 8, 204)
    land = m.land(tol=1.0, only={"CHN", "TWN"})
    pg.land(land, C["ochrel"], 0.55)
    with pg.clipped(land):
        # crowded plains and basins, as overlapping soft washes
        for lon, lat, rx, ry, c, op in ((116, 36, 9, 6, C["red"], 0.55), (113, 29, 7, 5, C["red"], 0.5),
                                       (120, 31, 4, 3, C["crimson"], 0.6), (105, 30, 4.5, 3.4, C["red"], 0.55),
                                       (113, 23, 4, 2.4, C["crimson"], 0.6), (110, 33, 6, 4, C["orange"], 0.5),
                                       (117, 26, 5, 3, C["orange"], 0.5), (125, 44, 5, 4, C["orange"], 0.45),
                                       (108, 25, 5, 3, C["orange"], 0.45), (121, 39, 3, 2, C["orange"], 0.5),
                                       (102, 36, 2.5, 1.6, C["orange"], 0.35), (87.6, 43.8, 1.6, 1, C["orange"], 0.35)):
            x, y = m(lon, lat)
            pg.wash(S(ell_pts(x, y, rx * m.k * m.sx, ry * m.sx, 14), True), c, op, edge=0)
        rng = random.Random(13)
        # a speckle of villages: many east of the line, a few in the oases of the west
        hu = lambda lon, lat: (lat - 25.0) * (127.5 - 98.5) - (lon - 98.5) * (50.2 - 25.0)
        east, west = [], []
        while len(east) < 160:
            lon, lat = rng.uniform(98, 126), rng.uniform(20, 48)
            if hu(lon, lat) < 0:
                east.append(m(lon, lat))
        while len(west) < 14:
            lon, lat = rng.uniform(75, 110), rng.uniform(28, 48)
            if hu(lon, lat) > 0:
                west.append(m(lon, lat))
        pg.dots(east, 0.5, C["crimson"], 0.7)
        pg.dots(west, 0.5, C["umber"], 0.6)
    pg.ink(land, 0.7)
    # the Hu Line, Heihe to Tengchong
    a, b = m(127.5, 50.2), m(98.5, 25.0)
    pg.ink(P([a, b]), 1.0, RED_PENCIL, 0.95, "3 2")
    pg.dot(a[0], a[1], 1.5, RED_PENCIL)
    pg.dot(b[0], b[1], 1.5, RED_PENCIL)
    # one inset split the same way: an empty steppe with a yurt | a crowded city block
    x0, y0, w, h = 218, 52, 94, 74
    with pg.inset_box(x0, y0, w, h):
        mid = x0 + w * 0.42
        pg.wash(rect(x0, y0, mid - x0, h), C["sky"], 0.45, edge=0)
        pg.wash(rect(x0, y0 + 40, mid - x0, h - 40), C["ochrel"], 0.7, edge=0)
        pg.ink(S([(x0, y0 + 38), (x0 + 16, y0 + 32), (mid, y0 + 36)]), 0.5, INK2)
        _yurt(pg, x0 + 20, y0 + 58, 0.9)
        pg.ink(P([(x0 + 36, y0 + 64), (x0 + 38, y0 + 62)]) + P([(x0 + 6, y0 + 66), (x0 + 8, y0 + 64)]), 0.5, INK2)
        # the city: towers and walk-ups packed together
        rng = random.Random(4)
        x = mid + 2
        bl = []
        while x < x0 + w - 2:
            bw = rng.uniform(5, 9)
            bh = rng.uniform(28, 62)
            bl.append((x, bw, bh))
            x += bw + 0.6
        for bx_, bw, bh in sorted(bl, key=lambda t: t[2]):
            top = y0 + h - bh
            pg.wash(rect(bx_, top, bw, bh), rng.choice([C["greyl"], C["ochrel"], C["sky"], C["umberl"]]), 0.8, edge=0)
        pg.ink("".join(P([(bx_, y0 + h), (bx_, y0 + h - bh), (bx_ + bw, y0 + h - bh), (bx_ + bw, y0 + h)]) for bx_, bw, bh in bl),
               0.45)
        pg.ink("".join(f"M{bx_ + 1.5:.1f} {yy}h{bw - 3:.1f}" for bx_, bw, bh in bl
                       for yy in range(int(y0 + h - bh + 4), int(y0 + h - 2), 5)), 0.3, INK2, 0.7, "0.8 1")
        pg.ink(P([(mid, y0), (mid, y0 + h)]), 0.9, RED_PENCIL, 0.95, "3 2")
    pg.leader(m(90, 38), (x0 + 20, y0), bend=-0.08)
    pg.leader(m(114, 31), (x0 + 70, y0 + h), bend=0.08)
    return pg.svg()
