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
def _figure(pg: Page, x, gy, kind):
    """Small wash-and-line figures for the stages of a life: toddler (sitting), child, adult, elder."""
    if kind == "toddler":
        pg.wash(S([(x - 4, gy), (x - 3.6, gy - 7), (x + 2.6, gy - 7.4), (x + 4, gy - 1), (x + 9, gy - 1), (x + 9, gy)], True),
                C["rose"], 0.7, edge=0)
        pg.ink(circ(x - 0.4, gy - 10.6, 3.2) + S([(x - 3.6, gy - 7), (x - 4, gy - 2), (x - 3, gy)]) +
               S([(x + 2.6, gy - 7.4), (x + 3.6, gy - 2), (x + 9, gy - 1)]) + P([(x - 2, gy), (x + 9, gy)]) +
               P([(x + 1.6, gy - 6.4), (x + 6, gy - 4)]), 0.65)
        return
    h = {"child": 21, "adult": 31, "elder": 29}[kind]
    s = h / 24
    lean = 2.4 * s if kind == "elder" else 0
    neck = (x + lean * .5, gy - h + 5.2 * s)
    hip = (x, gy - h * 0.44)
    col = {"child": C["ochre"], "adult": C["ultra"], "elder": C["violet"]}[kind]
    pg.wash(P([(neck[0] - 2.6 * s, neck[1] + .4), (neck[0] + 2.6 * s, neck[1] + .4), (hip[0] + 2.4 * s, hip[1] + 1),
               (hip[0] - 2.4 * s, hip[1] + 1)], True), col, 0.65, edge=0)
    pg.wash(circ(neck[0], gy - h + 2.6 * s, 2.5 * s), C["sand"], 0.7, edge=0)
    pg.ink(circ(neck[0], gy - h + 2.6 * s, 2.5 * s) + P([neck, hip]) + P([hip, (x - 2.6 * s, gy)]) + P([hip, (x + 2.6 * s, gy)]) +
           P([(neck[0], neck[1] + 1), (x - 3.6 * s, gy - h * 0.52)]) +
           P([(neck[0], neck[1] + 1), (x + 3.8 * s + lean, gy - h * 0.48)]), 0.7)
    if kind == "elder":
        pg.ink(P([(x + 3.8 * s + lean, gy - h * 0.48), (x + 5 * s + lean, gy)]), 0.75, C["umber"])


def life_expectancy():
    pg = Page(12)
    x0, x1 = 16, 206                     # the ruler runs from birth to 80 years
    span = x1 - x0

    def X(age):
        return x0 + span * age / 80
    gy = 78
    pg.ground(10, 212, gy + 3, C["sapl"], 4, 0.5)
    for age, kind in ((2, "toddler"), (10, "child"), (36, "adult"), (72, "elder")):
        _figure(pg, X(age), gy, kind)
    # a ruler: one tick a decade, longer at the half-way mark
    ry = 100
    pg.ink(P([(x0, ry), (x1, ry)]) + "".join(P([(X(a), ry), (X(a), ry - (4 if a % 40 == 0 else 2.4))]) for a in range(0, 81, 10)),
           0.6)
    # 1900: life was short; today: more than twice as long
    pg.wash(rect(x0, 110, X(32) - x0, 8), C["greyl"], 0.85)
    pg.ink(rect(x0, 110, X(32) - x0, 8), 0.55)
    pg.wash(rect(x0, 128, X(73) - x0, 8), C["sap"], 0.8)
    pg.ink(rect(x0, 128, X(73) - x0, 8), 0.55)
    pg.ink(P([(X(32), 104), (X(32), 140)]) + P([(X(73), 104), (X(73), 140)]), 0.4, INK2, 0.8, "1.4 1.4")
    pg.arrow([(X(32) + 2, 114), (X(52), 120), (X(73) - 2, 132)], RED_PENCIL, 1.0, 3.2)
    # the table has only country codes, so it is joined to the countries: a tiny table -> a map
    tx, ty = 232, 18
    with pg.inset_box(tx, ty, 26, 36):
        for i in range(6):
            yy = ty + 3 + i * 5.5
            pg.ink(P([(tx + 3, yy + 2), (tx + 9, yy + 2)]), 1.3, INK2, 0.7)
            pg.wash(rect(tx + 13, yy, 10, 4), mix(C["ochrel"], C["hooker"], i / 5), 0.9, edge=0)
        pg.ink(P([(tx + 11, ty), (tx + 11, ty + 36)]), 0.4, INK2)
    pg.arrow([(tx + 30, ty + 16), (tx + 46, ty + 22), (tx + 50, ty + 44)], RED_PENCIL, 1.0, 3)
    m = Equi(-170, 180, -56, 76, 228, 66, 84)
    with pg.inset_box(228, 66, 84, m.h):
        pg.wash(rect(228, 66, 84, m.h), C["sky"], 0.35, edge=0)
        shade = {"Africa": 0.15, "Asia": 0.55, "South America": 0.6, "North America": 0.75, "Europe": 0.9,
                 "Oceania": 0.85}
        for cont, t in shade.items():
            isos = {i for i, _ in countries() if continent(i) == cont}
            pg.land(m.land(tol=1.0, only=isos), mix(C["ochrel"], C["hooker"], t), 0.9, 0.8)
    # a tiny world map on the right edge of the table's arrow, plus the same shading as a key
    for i in range(5):
        pg.wash(rect(236 + i * 14, 150, 13, 6), mix(C["ochrel"], C["hooker"], i / 4), 0.9, edge=0)
    pg.ink(rect(236, 150, 70, 6), 0.45)
    pg.arrow([(236, 162), (306, 162)], INK, 0.6, 2.2, smooth=False)
    return pg.svg()


# ------------------------------------------------------------------ population (WorldPop grid, the Hu Line)
def _yurt(pg: Page, cx, cy, s):
    wall = [(cx - 12 * s, cy), (cx - 12 * s, cy - 7 * s), (cx + 12 * s, cy - 7 * s), (cx + 12 * s, cy)]
    pg.wash(P(wall, True), C["white"], 0.9, edge=0.5)
    pg.wash(P([(cx - 13 * s, cy - 7 * s), (cx, cy - 15 * s), (cx + 13 * s, cy - 7 * s)], True), C["greyl"], 0.85)
    pg.ink(P(wall) + P([(cx - 13 * s, cy - 7 * s), (cx, cy - 15 * s), (cx + 13 * s, cy - 7 * s)]) +
           rect(cx - 2.6 * s, cy - 6 * s, 5.2 * s, 6 * s), 0.6)
    pg.wash(rect(cx - 2.6 * s, cy - 6 * s, 5.2 * s, 6 * s), C["red"], 0.8, edge=0)
    pg.ink("".join(P([(cx + k * 3 * s, cy - 7 * s), (cx + k * 3 * s, cy)]) for k in (-3, -2, 2, 3)), 0.3, INK2)


def _village(pg: Page, cx, cy, s):
    rng = random.Random(5)
    for row in range(3):
        for i in range(5):
            x = cx - 22 * s + i * 10 * s + (row % 2) * 5 * s
            y = cy - 14 * s + row * 9 * s
            w = 8 * s
            pg.wash(rect(x, y, w, 5 * s), C["white"], 0.9, edge=0.5)
            roof = [(x - 1 * s, y), (x + w / 2, y - 3.5 * s), (x + w + 1 * s, y)]
            pg.wash(P(roof, True), rng.choice([C["slate"], C["payne"], C["bsienna"]]), 0.75, edge=0)
            pg.ink(P(roof) + P([(x, y), (x, y + 5 * s), (x + w, y + 5 * s), (x + w, y)]), 0.45)


def population():
    pg = Page(13)
    m = Equi(73, 135, 17, 54, 10, 10, 186)
    rings_ll = country_rings("CHN") + country_rings("TWN")
    land = m.land(tol=1.0, only={"CHN", "TWN"})
    pg.land(land, C["ochrel"], 0.5)
    # a coarse version of the 1 km grid: each cell coloured by people per cell (from the bundled cities)
    cs = 4.2
    pops = [(lon, lat, pop) for lon, lat, pop, _ in cities() if 70 <= lon <= 136 and 15 <= lat <= 55 and pop > 0]
    cells = {1: "", 2: "", 3: "", 4: ""}
    yy = m.y
    while yy < m.y + m.h:
        xx = m.x
        while xx < m.x + 186:
            lon = m.lon0 + (xx + cs / 2 - m.x) / (m.k * m.sx)
            lat = m.lat1 - (yy + cs / 2 - m.y) / m.sx
            if inside((lon, lat), rings_ll):
                v = 0.0
                for plon, plat, pop in pops:
                    d2 = (plon - lon) ** 2 + (plat - lat) ** 2
                    v += pop / 1e6 * math.exp(-d2 / 6.0)
                # the plains east of the line are farmed and crowded even away from cities
                hu = (lat - 25.0) * (127.5 - 98.5) - (lon - 98.5) * (50.2 - 25.0)
                v += 0.9 if hu < 0 else 0.03
                k = 4 if v > 6 else 3 if v > 2.2 else 2 if v > 0.8 else 1
                cells[k] += rect(xx + .3, yy + .3, cs - .6, cs - .6)
            xx += cs
        yy += cs
    cols = {1: C["ochrel"], 2: C["orange"], 3: C["red"], 4: C["crimson"]}
    for k in (1, 2, 3, 4):
        if cells[k]:
            pg.wash(cells[k], cols[k], 0.25 + 0.17 * k, edge=0)
    pg.ink(land, 0.55)
    # the Hu Line, Heihe to Tengchong
    a, b = m(127.5, 50.2), m(98.5, 25.0)
    pg.ink(P([a, b]), 0.9, RED_PENCIL, 0.95, "3 2")
    pg.dot(a[0], a[1], 1.4, RED_PENCIL)
    pg.dot(b[0], b[1], 1.4, RED_PENCIL)
    # either side of the line, close up: a yurt on the empty west, a crowded village on the east
    with pg.inset_circle(262, 46, 30):
        pg.wash(circ(262, 46, 30), C["sky"], 0.35, edge=0)
        pg.wash(rect(232, 52, 60, 30), C["ochrel"], 0.6, edge=0)
        pg.ink(S([(232, 44), (250, 38), (270, 42), (292, 36)]), 0.5, INK2)
        _yurt(pg, 262, 58, 1.0)
    with pg.inset_circle(262, 128, 30):
        pg.wash(circ(262, 128, 30), C["sapl"], 0.5, edge=0)
        _village(pg, 262, 132, 1.0)
    w_pt, e_pt = m(90, 36), m(116, 30)
    pg.leader(w_pt, (233, 40), bend=-0.08)
    pg.leader(e_pt, (233, 132), bend=0.08)
    return pg.svg()
