"""OpenStreetMap and other web-collected points: chain stores, overpass turbo, BBBike and Geofabrik
extracts, Singapore's hawker centres."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, Equi, P, S, circ, ell, ell_pts, rect, scallop,
                  lerp, lerp2, mix, inside)


def _street_map(pg: Page, x, y, w, h, seed, river=True):
    """A small hand-drawn street map: a few curving avenues, a grid of side streets, a river."""
    rng = random.Random(seed)
    pg.wash(rect(x, y, w, h), C["white"], 0.6, edge=0)
    if river:
        rv = [(x, y + h * 0.7), (x + w * 0.3, y + h * 0.62), (x + w * 0.6, y + h * 0.75), (x + w, y + h * 0.66)]
        pg.stroke_wash(S(rv), C["cerul"], 4.0, 0.7)
    d = ""
    for k in range(1, 6):
        yy = y + h * k / 6 + rng.uniform(-2, 2)
        d += S([(x, yy), (x + w * 0.5, yy + rng.uniform(-3, 3)), (x + w, yy + rng.uniform(-2, 2))])
    for k in range(1, 7):
        xx = x + w * k / 7 + rng.uniform(-2, 2)
        d += S([(xx, y), (xx + rng.uniform(-3, 3), y + h * 0.5), (xx + rng.uniform(-2, 2), y + h)])
    pg.ink(d, 0.35, INK2, 0.8)
    pg.ink(S([(x, y + h * 0.15), (x + w * 0.45, y + h * 0.45), (x + w, y + h * 0.5)]) +
           S([(x + w * 0.2, y + h), (x + w * 0.5, y + h * 0.45), (x + w * 0.75, y)]), 1.1, C["ochre"], 0.85)


# ------------------------------------------------------------------ chain stores
def chain_stores():
    pg = Page(30)
    gy = 140
    pg.vignette(4, gy, 214, 166, C["greyl"], 0.55, 2.5, seed=1)
    pg.ink(P([(4, gy), (214, gy)]) + P([(4, gy + 14), (214, gy + 14)]), 0.6)
    # a row of shopfronts: two chains (green and red awnings) among independent shops
    shops = [(8, 36, "g"), (46, 30, None), (78, 38, "r"), (118, 30, None), (150, 36, "g"), (188, 24, None)]
    for x, w, brand in shops:
        top = gy - 72 + (x % 7)
        pg.wash(rect(x, top, w, gy - top), C["white"] if brand else C["ochrel"], 0.7, edge=0.5)
        pg.ink(rect(x, top, w, gy - top), 0.55)
        # upper-floor windows
        for wx in (x + 4, x + w - 12):
            pg.wash(rect(wx, top + 8, 8, 12), C["sky"], 0.6, edge=0)
            pg.ink(rect(wx, top + 8, 8, 12) + P([(wx + 4, top + 8), (wx + 4, top + 20)]), 0.4)
        # sign board (blank, a brand emblem only) and awning
        if brand:
            col = C["hooker"] if brand == "g" else C["red"]
            pg.wash(rect(x + 2, gy - 44, w - 4, 7), col, 0.85)
            pg.ink(rect(x + 2, gy - 44, w - 4, 7), 0.5)
            if brand == "g":
                pg.raw(f'<circle cx="{x + w / 2}" cy="{gy - 40.5}" r="2.4" fill="{C["white"]}"/>')
            else:
                pg.raw(f'<path d="M{x + w / 2 - 3} {gy - 38}l3-5 3 5z" fill="{C["lemon"]}"/>')
            aw = [(x, gy - 37), (x + w, gy - 37), (x + w + 3, gy - 29), (x - 3, gy - 29)]
            pg.wash(P(aw, True), col, 0.6)
            pg.ink(P(aw, True) + "".join(P([(x + i * w / 6, gy - 37), (x - 3 + i * (w + 6) / 6, gy - 29)]) for i in range(1, 6)),
                   0.4)
        else:
            pg.wash(rect(x + 2, gy - 42, w - 4, 6), C["umberl"], 0.7, edge=0)
        # shop window and door
        pg.wash(rect(x + 3, gy - 26, w - 15, 18), C["sky"], 0.5, edge=0)
        pg.ink(rect(x + 3, gy - 26, w - 15, 18) + rect(x + w - 10, gy - 24, 7, 24), 0.45)
    pg.person(70, gy + 8, 13)
    pg.person(140, gy + 10, 12)
    # the map: each chain's branches across a city
    with pg.inset_box(230, 18, 82, 82):
        _street_map(pg, 230, 18, 82, 82, 30)
        rng = random.Random(3)
        for c, n_, cx, cy, spread in ((C["hooker"], 14, 262, 52, 18), (C["red"], 9, 284, 70, 24)):
            for _ in range(n_):
                x, y = cx + rng.gauss(0, spread * 0.6), cy + rng.gauss(0, spread * 0.6)
                x, y = min(max(x, 234), 308), min(max(y, 22), 96)
                pg.raw(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="2" fill="{c}" stroke="{INK}" stroke-width=".4"/>')
    pg.leader((26, gy - 40), (230, 50), bend=-0.12)
    pg.leader((97, gy - 40), (230, 82), bend=-0.08)
    # and the counts, compared: a quick bar for each chain
    for i, (c, L) in enumerate(((C["hooker"], 70), (C["red"], 46))):
        pg.wash(rect(236, 124 + i * 14, L, 7), c, 0.8)
        pg.ink(rect(236, 124 + i * 14, L, 7), 0.5)
    pg.ink(P([(236, 118), (236, 154)]), 0.5)
    return pg.svg()


# ------------------------------------------------------------------ overpass turbo (a Paris drinking fountain)
def openstreetmap():
    pg = Page(31)
    gy = 158
    cx = 90
    green = C["hooker"]
    pg.ground(30, 170, gy + 3, C["greyl"], 4, 0.6)
    # pedestal: stepped base and an octagonal plinth
    pg.wash(rect(cx - 28, gy - 6, 56, 6), green, 0.75)
    pg.wash(rect(cx - 20, gy - 30, 40, 24), green, 0.75)
    pg.wash(rect(cx + 6, gy - 30, 14, 24), "#3b3f47", 0.35, edge=0)
    pg.ink(rect(cx - 28, gy - 6, 56, 6) + rect(cx - 20, gy - 30, 40, 24) + P([(cx - 6, gy - 30), (cx - 6, gy - 6)]) +
           P([(cx + 6, gy - 30), (cx + 6, gy - 6)]), 0.7)
    pg.ink(rect(cx - 16, gy - 24, 8, 12) + rect(cx + 8, gy - 24, 8, 12), 0.4)
    pg.wash(rect(cx - 24, gy - 34, 48, 4), green, 0.85)
    pg.ink(rect(cx - 24, gy - 34, 48, 4), 0.7)
    # four caryatids standing back to back, holding up the dome (two seen in front, two behind)
    top = gy - 98
    for k, (fx, sc, front) in enumerate(((cx - 13, 1.0, True), (cx + 13, 1.0, True), (cx - 4, 0.9, False), (cx + 4, 0.9, False))):
        if front:
            continue
        pg.wash(P([(fx - 4, gy - 34), (fx - 3, top + 14), (fx + 3, top + 14), (fx + 4, gy - 34)], True), green, 0.5, edge=0)
    for fx in (cx - 13, cx + 13):
        robe = [(fx - 6, gy - 34), (fx - 4, gy - 60), (fx - 3.4, top + 20), (fx + 3.4, top + 20), (fx + 4, gy - 60), (fx + 6, gy - 34)]
        pg.wash(S(robe, True, 0.25), green, 0.8)
        pg.ink(S(robe, False, 0.25), 0.6)
        pg.ink("".join(S([(fx + dx, gy - 36), (fx + dx * 0.7, gy - 56), (fx + dx * 0.5, top + 24)]) for dx in (-2.5, 0, 2.5)), 0.3)
        pg.wash(circ(fx, top + 15, 3.4), green, 0.85)
        pg.ink(circ(fx, top + 15, 3.4), 0.6)
        # arms raised to the dome's rim
        pg.ink(S([(fx - 3.4, top + 21), (fx - 6, top + 14), (fx - 5, top + 8)]) + S([(fx + 3.4, top + 21), (fx + 6, top + 14), (fx + 5, top + 8)]),
               0.7)
    # the dome: a cupola with a scale pattern, a finial on top; water falls from its centre
    dome = f"M{cx - 24} {top + 8}Q{cx} {top - 26} {cx + 24} {top + 8}Z"
    pg.wash(dome, green, 0.85)
    pg.ink(dome + P([(cx - 26, top + 8), (cx + 26, top + 8)]) + rect(cx - 25, top + 8, 50, 3), 0.7)
    pg.ink("".join(f"M{cx + dx - 3} {top + y}q3 3 6 0" for y, dxs in ((4, (-14, -7, 0, 7, 14)), (-2, (-10, -3, 4, 11)), (-7, (-5, 2))) for dx in dxs),
           0.35, INK2)
    pg.ink(P([(cx, top - 9), (cx, top - 18)]) + circ(cx, top - 20, 2), 0.7)
    pg.ink(P([(cx, top + 11), (cx, gy - 34)]), 0.8, C["cerul"], 0.9, "3 1.6")
    pg.person(146, gy, 68, C["slate"])     # a Wallace fountain is about 2.7 m tall
    pg.scalebar(164, gy + 8, 40, 4)
    # the query's answer: every drinking-water point on a street map
    with pg.inset_box(214, 18, 98, 96):
        _street_map(pg, 214, 18, 98, 96, 31)
        rng = random.Random(4)
        d = ""
        for _ in range(22):
            x, y = rng.uniform(220, 306), rng.uniform(26, 108)
            d += f"M{x:.1f} {y - 3.4:.1f}c1.6 2 2.2 3 2.2 4a2.2 2.2 0 0 1 -4.4 0c0-1 .6-2 2.2-4z"
        pg.ink_fill(d, C["cerul"])
        pg.ink(d, 0.35)
    pg.leader((cx + 24, top + 4), (214, 54), bend=-0.08)
    return pg.svg()


# ------------------------------------------------------------------ BBBike city extract
def city_extract():
    pg = Page(32)
    rng = random.Random(32)

    def O(x, y):           # plan -> oblique (a quick bird's-eye)
        return (10 + x * 0.72 + y * 0.3, 166 - y * 0.6)
    # ground, river, park
    pg.wash(P([O(0, 0), O(170, 0), O(170, 210), O(0, 210)], True), C["sand"], 0.35, edge=0)
    river = [O(0, 70), O(50, 84), O(100, 72), O(170, 96)]
    pg.stroke_wash(S(river), C["cerul"], 7.0, 0.7)
    park = [O(110, 130), O(150, 130), O(150, 170), O(110, 170)]
    pg.wash(P(park, True), C["sapl"], 0.8)
    for i in range(6):
        x, y = O(116 + (i % 3) * 13, 138 + (i // 3) * 18)
        pg.wash(circ(x, y - 3, 3.2), C["hooker"], 0.75, edge=0)
    # streets on a grid, and building blocks as little extruded boxes
    d = ""
    for k in range(0, 211, 30):
        d += P([O(0, k), O(170, k)])
    for k in range(0, 171, 34):
        d += P([O(k, 0), O(k, 210)])
    pg.ink(d, 0.45, INK2, 0.8)
    for gx in range(0, 170, 34):
        for gy_ in range(0, 210, 30):
            if 60 <= gy_ < 90 or (110 <= gx < 150 and 120 <= gy_ < 180):
                continue
            for i in range(2):
                bx, by = gx + 4 + i * 14, gy_ + 5
                bw, bd, bh = 11, 19, rng.uniform(4, 11)
                base = [O(bx, by), O(bx + bw, by), O(bx + bw, by + bd), O(bx, by + bd)]
                roof = [(p[0], p[1] - bh) for p in base]
                pg.wash(P([base[0], base[1], roof[1], roof[0]], True), C["greyl"], 0.85, edge=0)
                pg.wash(P([base[1], base[2], roof[2], roof[1]], True), C["slate"], 0.55, edge=0)
                pg.wash(P(roof, True), C["white"], 0.9, edge=0)
                pg.ink(P(roof, True) + P([base[0], roof[0]]) + P([base[1], roof[1]]) + P([base[0], base[1]]), 0.3)
    # the box you draw around the city, with its corner handles
    sel = [O(-4, -4), O(174, -4), O(174, 214), O(-4, 214)]
    pg.ink(P(sel, True), 0.9, RED_PENCIL, 0.95, "4 2.5")
    for p in sel:
        pg.raw(f'<rect x="{p[0] - 2.2:.1f}" y="{p[1] - 2.2:.1f}" width="4.4" height="4.4" fill="{C["white"]}" stroke="{RED_PENCIL}" stroke-width=".8"/>')
    # what comes back: three layers peeled apart, then an envelope when it is ready
    for k, (kind, y) in enumerate((("roads", 26), ("buildings", 60), ("shops", 94))):
        with pg.inset_box(256, y, 50, 26):
            if kind == "roads":
                pg.ink(S([(256, y + 20), (276, y + 12), (306, y + 14)]) + P([(270, y), (282, y + 26)]) +
                       P([(292, y), (288, y + 26)]), 0.9, C["ochre"])
            elif kind == "buildings":
                for i in range(6):
                    pg.wash(rect(260 + i * 7.6, y + 6 + (i % 2) * 6, 6, 8), C["slate"], 0.7)
            else:
                for i in range(7):
                    pg.raw(f'<circle cx="{262 + i * 6.6:.1f}" cy="{y + 9 + (i % 3) * 4:.1f}" r="1.6" fill="{C["red"]}"/>')
    pg.ink(P([(240, 39), (256, 39)]) + P([(240, 73), (256, 73)]) + P([(240, 107), (256, 107)]) + P([(240, 39), (240, 107)]),
           0.5, INK2)
    pg.leader(O(174, 214), (240, 73), dot_a=True, bend=0.05)
    ex, ey = 268, 140
    pg.wash(rect(ex, ey, 30, 20), C["white"], 0.9, edge=0.5)
    pg.ink(rect(ex, ey, 30, 20) + P([(ex, ey), (ex + 15, ey + 11), (ex + 30, ey)]), 0.7)
    pg.arrow([(283, 118), (283, 136)], RED_PENCIL, 0.9, 3, smooth=False)
    return pg.svg()


# ------------------------------------------------------------------ Geofabrik country extract
def country_extract():
    pg = Page(33)
    m = Equi(5.5, 15.5, 47, 55.2, 30, 20, 104)
    land = m.land(tol=0.8, only={"DEU"})
    pg.land(land, C["sapl"], 0.65)
    pg.ink(land, 0.75)
    rng = random.Random(33)
    # roads, railways, towns drawn on the map
    towns = [(13.4, 52.5), (10.0, 53.55), (11.6, 48.15), (6.95, 50.94), (8.68, 50.11), (9.18, 48.78), (6.78, 51.23),
             (12.37, 51.34), (13.74, 51.05), (8.8, 53.08), (9.73, 52.37), (11.08, 49.45), (8.4, 49.0), (7.1, 50.73)]
    tp = [m(*t) for t in towns]
    roads = ""
    for a, b in ((0, 1), (1, 9), (9, 10), (10, 0), (10, 4), (4, 5), (5, 2), (2, 11), (11, 7), (7, 0), (3, 6), (3, 13),
                 (13, 4), (6, 10), (7, 8), (8, 12), (12, 5), (11, 4)):
        pa, pb = tp[a], tp[b]
        mid = lerp2(pa, pb, 0.5)
        roads += S([pa, (mid[0] + rng.uniform(-3, 3), mid[1] + rng.uniform(-3, 3)), pb])
    pg.ink(roads, 0.8, C["ochre"], 0.9)
    rails = ""
    for a, b in ((0, 7), (1, 3), (3, 4), (4, 2), (0, 12)):
        pa, pb = tp[a], tp[b]
        rails += S([pa, lerp2(pa, pb, 0.5), pb])
    pg.ink(rails, 0.6, INK, 0.85)
    pg.ink(rails, 0.6, C["white"], 0.9, "1.6 1.6")
    for x, y in tp:
        pg.raw(f'<rect x="{x - 1.5:.1f}" y="{y - 1.5:.1f}" width="3" height="3" fill="{C["crimson"]}" stroke="{INK}" stroke-width=".4"/>')
    # the free download splits it into layers: roads, railways, buildings, places of worship
    boxes = [(176, 16), (246, 16), (176, 96), (246, 96)]
    for k, (bx, by) in enumerate(boxes):
        with pg.inset_box(bx, by, 64, 64):
            pg.wash(rect(bx, by, 64, 64), C["white"], 0.5, edge=0)
            if k == 0:
                pg.ink(S([(bx, by + 50), (bx + 30, by + 30), (bx + 64, by + 22)]) + S([(bx + 12, by), (bx + 26, by + 34), (bx + 20, by + 64)]) +
                       S([(bx + 40, by + 64), (bx + 44, by + 30), (bx + 60, by)]), 1.3, C["ochre"])
                pg.ink(S([(bx, by + 14), (bx + 30, by + 20), (bx + 64, by + 44)]), 0.6, C["ochre"])
            elif k == 1:
                tr = S([(bx, by + 44), (bx + 30, by + 30), (bx + 64, by + 26)]) + S([(bx + 24, by), (bx + 30, by + 30), (bx + 40, by + 64)])
                pg.ink(tr, 1.2, INK)
                pg.ink(tr, 1.0, C["white"], 1.0, "2.4 2.4")
            elif k == 2:
                for i in range(18):
                    x, y = bx + 6 + (i % 6) * 9.6 + rng.uniform(-1, 1), by + 8 + (i // 6) * 17 + rng.uniform(-2, 2)
                    w, h = rng.uniform(4, 7), rng.uniform(4, 9)
                    pg.wash(rect(x, y, w, h), C["slate"], 0.75, edge=0.5)
            else:
                for x, y in ((bx + 16, by + 24), (bx + 44, by + 18), (bx + 32, by + 48), (bx + 52, by + 50)):
                    # a small church: nave and a spire
                    pg.wash(rect(x - 4, y - 4, 10, 6) + f"M{x - 4} {y - 4}l2-6 2 6z", C["crimson"], 0.75)
                    pg.ink(rect(x - 4, y - 4, 10, 6) + f"M{x - 4} {y - 4}l2-6 2 6z", 0.5)
    pg.leader(tp[5], (176, 48), bend=-0.1)
    pg.leader(tp[0], (246, 30), bend=-0.15)
    return pg.svg()


# ------------------------------------------------------------------ hawker centres
SG = [(103.64, 1.32), (103.67, 1.27), (103.74, 1.26), (103.80, 1.26), (103.86, 1.27), (103.90, 1.30), (103.97, 1.31),
      (104.03, 1.33), (104.0, 1.38), (103.95, 1.40), (103.88, 1.43), (103.80, 1.45), (103.72, 1.44), (103.67, 1.40),
      (103.64, 1.36)]


def hawker():
    pg = Page(34)
    gy = 150
    pg.vignette(4, gy - 2, 214, 168, C["greyl"], 0.55, 2.5, seed=1)
    # open-sided pavilion: a big pitched roof on columns that stand on the floor and reach the eaves
    eave_y, ridge_y = 70, 40
    roof = [(8, eave_y + 6), (106, ridge_y), (204, eave_y + 6), (196, eave_y + 12), (106, ridge_y + 8), (16, eave_y + 12)]
    pg.wash(P(roof, True), C["bsienna"], 0.75)
    pg.ink(P(roof, True) + P([(106, ridge_y), (106, ridge_y + 8)]), 0.75)
    # a raised clerestory along the ridge for ventilation
    pg.wash(P([(86, ridge_y - 4), (126, ridge_y - 4), (126, ridge_y + 3), (86, ridge_y + 3)], True), C["bsienna"], 0.8)
    pg.ink(P([(80, ridge_y - 1), (106, ridge_y - 10), (132, ridge_y - 1)]) + P([(86, ridge_y - 3), (86, ridge_y + 3)]) +
           P([(126, ridge_y - 3), (126, ridge_y + 3)]), 0.6)
    # columns: underside of the eave at each column's x, down to the floor
    def under(x):          # the roof's underside (from (16, 82) up to the ridge at (106, 48))
        return eave_y + 12 - (90 - abs(x - 106)) * (34 / 90)
    cols = (24, 64, 106, 148, 188)
    for x in cols:
        u = under(x)
        pg.wash(rect(x - 1.6, u, 3.2, gy - u), C["greyl"], 0.9, edge=0)
        pg.ink(P([(x - 1.6, u), (x - 1.6, gy)]) + P([(x + 1.6, u), (x + 1.6, gy)]), 0.6)
    # stalls along the back: counters with shutters, each stall a different colour
    for i, c in enumerate((C["ochre"], C["sap"], C["red"], C["cerul"], C["orange"])):
        x = 30 + i * 32
        pg.wash(rect(x, 104, 26, 20), c, 0.55)
        pg.ink(rect(x, 104, 26, 20) + P([(x, 112), (x + 26, 112)]), 0.45)
        pg.wash(rect(x + 2, 98, 22, 5), C["white"], 0.9, edge=0.5)
    # ceiling fans and round tables with stools
    for x in (60, 106, 152):
        pg.ink(P([(x, under(x)), (x, 84)]) + P([(x - 9, 85), (x + 9, 83)]), 0.6)
    for x in (44, 92, 140, 180):
        pg.wash(ell(x, 134, 10, 2.6), C["white"], 0.9, edge=0.6)
        pg.ink(ell(x, 134, 10, 2.6) + P([(x, 136.6), (x, gy)]), 0.6)
        for sx in (-14, 14):
            pg.ink(ell(x + sx, 142, 3, 1) + P([(x + sx, 143), (x + sx, gy)]), 0.5)
    pg.person(120, gy, 20)
    # inset: a plate of chicken rice with chopsticks (a small, tasty first map)
    with pg.inset_circle(268, 50, 32):
        pg.wash(circ(268, 50, 32), C["sapl"], 0.4, edge=0)
        pg.wash(ell(266, 56, 22, 9), C["white"], 0.95, edge=0.6)
        pg.ink(ell(266, 56, 22, 9) + ell(266, 55, 16, 6), 0.6)
        pg.wash(S([(254, 54), (258, 46), (268, 44), (276, 48), (272, 56), (260, 58)], True), C["white"], 0.95, edge=0.6)
        pg.ink(scallop([(254, 54), (258, 46), (268, 44), (276, 48)], 0.25), 0.5)
        for k in range(4):
            sl = [(266 + k * 4, 50), (272 + k * 4, 49), (271 + k * 4, 58), (265 + k * 4, 58)]
            pg.wash(P(sl, True), C["ochrel"], 0.85)
            pg.ink(P(sl, True), 0.4)
        pg.wash(S(ell_pts(280, 58, 3, 2, 8), True), C["sap"], 0.8, edge=0)
        pg.ink(P([(244, 70), (292, 32)]) + P([(248, 72), (294, 36)]), 0.9, C["umber"])
    pg.leader((76, 112), (238, 60), bend=-0.1)
    # all the hawker centres on the island
    m = Equi(103.6, 104.08, 1.2, 1.49, 230, 104, 82)
    with pg.inset_box(230, 104, 82, m.h):
        pg.wash(rect(230, 104, 82, m.h), C["sky"], 0.5, edge=0)
        isl = [m(*p) for p in SG]
        pg.land(S(isl, True, 0.4), C["sapl"], 0.9)
        pg.ink(S(isl, True, 0.4), 0.6)
        rng = random.Random(8)
        pts = []
        for _ in range(60):
            lon, lat = rng.uniform(103.68, 104.0), rng.uniform(1.27, 1.43)
            if inside((lon, lat), [SG]):
                pts.append(m(lon, lat))
        pg.dots(pts, 1.1, C["red"])
    return pg.svg()
