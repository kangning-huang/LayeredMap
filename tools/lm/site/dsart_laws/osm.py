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
    """A high street sketched from across the road: two chains (green and red signs) among
    independent shops; the chains' branches, city by city, as two small map insets."""
    pg = Page(30)
    gy = 146
    # pavement, kerb and road
    pg.vignette(4, gy, 226, 158, C["greyl"], 0.6, 2, seed=1)
    pg.vignette(6, 160, 224, 174, C["payne"], 0.3, 2.5, seed=2)
    pg.ink(P([(4, gy), (226, gy)]) + P([(4, 158), (226, 158)]), 0.7)
    pg.ink("".join(f"M{x} 167h8" for x in range(14, 220, 22)), 0.8, C["white"], 0.9)
    shops = [(6, 38, "g", 70), (46, 30, None, 82), (78, 40, "r", 66), (120, 30, None, 78), (152, 38, "g", 72),
             (192, 34, None, 84)]
    for x, w, brand, hh in shops:
        top = gy - hh
        pg.wash(rect(x, top, w, hh), C["white"] if brand else C["ochrel"], 0.75, edge=0.5)
        pg.ink(rect(x, top, w, hh), 0.6)
        # cornice and upper-floor windows
        pg.ink(P([(x - 1.5, top + 3), (x + w + 1.5, top + 3)]), 0.9)
        for wx in (x + 5, x + w - 13):
            for wy in (top + 9, top + 24) if hh > 75 else (top + 10,):
                pg.wash(rect(wx, wy, 8, 11), C["sky"], 0.7, edge=0)
                pg.ink(rect(wx, wy, 8, 11) + P([(wx + 4, wy), (wx + 4, wy + 11)]) + P([(wx - 1, wy + 11), (wx + 9, wy + 11)]), 0.45)
        if brand:
            col = C["hooker"] if brand == "g" else C["red"]
            pg.wash(rect(x + 2, gy - 44, w - 4, 8), col, 0.9)
            pg.ink(rect(x + 2, gy - 44, w - 4, 8), 0.55)
            if brand == "g":
                pg.raw(f'<circle cx="{x + w / 2}" cy="{gy - 40}" r="2.6" fill="{C["white"]}"/>')
            else:
                pg.raw(f'<path d="M{x + w / 2 - 3.2} {gy - 37.5}l3.2-5 3.2 5z" fill="{C["lemon"]}"/>')
            aw = [(x, gy - 36), (x + w, gy - 36), (x + w + 3, gy - 28), (x - 3, gy - 28)]
            pg.wash(P(aw, True), col, 0.65)
            pg.ink(P(aw, True) + "".join(P([(x + i * w / 6, gy - 36), (x - 3 + i * (w + 6) / 6, gy - 28)]) for i in range(1, 6)), 0.45)
        else:
            pg.wash(rect(x + 2, gy - 42, w - 4, 6), C["umberl"], 0.75, edge=0)
            pg.ink(rect(x + 2, gy - 42, w - 4, 6), 0.4)
        pg.wash(rect(x + 3, gy - 25, w - 15, 18), C["sky"], 0.55, edge=0)
        pg.ink(rect(x + 3, gy - 25, w - 15, 18) + rect(x + w - 10, gy - 24, 7, 24), 0.5)
    # street furniture and people, for scale
    pg.ink(P([(116, gy + 6), (116, gy - 66)]) + S([(116, gy - 66), (118, gy - 70), (124, gy - 70)]), 0.9)
    pg.wash(ell(125, gy - 68.5, 3, 1.6), C["lemon"], 0.9)
    pg.person(64, gy + 8, 21)
    pg.person(140, gy + 10, 20)
    pg.person(176, gy + 9, 14)
    # the same two chains, branch by branch, in two cities
    for k, (cx, cy) in enumerate(((272, 46), (272, 124))):
        with pg.inset_circle(cx, cy, 32):
            _street_map(pg, cx - 32, cy - 32, 64, 64, 30 + k, river=(k == 0))
            rng = random.Random(3 + k)
            for c, n_, sp in ((C["hooker"], 9 if k == 0 else 5, 13), (C["red"], 5 if k == 0 else 9, 16)):
                for _ in range(n_):
                    x, y = cx + rng.gauss(0, sp), cy + rng.gauss(0, sp)
                    pg.raw(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="2" fill="{c}" stroke="{INK}" stroke-width=".4"/>')
    pg.leader((25, gy - 40), (241, 52), bend=-0.14)
    pg.leader((98, gy - 40), (241, 116), bend=-0.08)
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
    """A German river-valley landscape: a village with its church, an autobahn on the hillside, a
    railway along the river. The download splits a country into exactly these layers (roads,
    railways, buildings, places of worship), drawn as four small cards beside a Germany outline."""
    pg = Page(33)
    # far hills with vineyards, a castle on the ridge
    far = [(4, 70), (34, 56), (66, 62), (96, 48), (130, 58), (168, 50), (206, 62)]
    pg.wash(S(far + [(206, 96), (4, 96)], True), C["sapl"], 0.55, edge=0)
    pg.ink(S(far), 0.6, INK2)
    pg.ink("".join(P([(x, 70 - (x % 9)), (x + 6, 86 - (x % 9))]) for x in range(20, 80, 6)), 0.3, C["hooker"], 0.8)
    kx, ky = 96, 52
    pg.wash(rect(kx - 4, ky - 12, 8, 12) + rect(kx + 4, ky - 6, 10, 6), C["greyl"], 0.9)
    pg.ink(rect(kx - 4, ky - 12, 8, 12) + rect(kx + 4, ky - 6, 10, 6) + P([(kx - 5, ky - 12), (kx, ky - 17), (kx + 5, ky - 12)]), 0.55)
    # river in the valley floor
    river = [(4, 132), (60, 126), (120, 134), (180, 128), (214, 132)]
    pg.wash(S(river + [(214, 140), (180, 137), (120, 143), (60, 135), (4, 141)], True), C["cerul"], 0.65, edge=0)
    pg.ink(S(river), 0.5, INK2)
    pg.vignette(4, 92, 214, 128, C["sapl"], 0.45, 3, seed=4)
    pg.vignette(4, 141, 214, 172, C["sapl"], 0.55, 3, seed=5)
    # autobahn across the hillside, with a bridge pier pair
    road = [(4, 104), (70, 98), (140, 106), (214, 100)]
    pg.ink(S(road), 3.2, C["greyl"], 0.95)
    pg.ink(S([(x, y - 1.6) for x, y in road]) + S([(x, y + 1.6) for x, y in road]), 0.5)
    pg.ink(S(road), 0.4, C["white"], 0.9, "2 2")
    # railway along the river, with a short train
    rail = [(4, 150), (80, 146), (150, 152), (214, 148)]
    pg.ink(S(rail), 1.2, INK)
    pg.ink(S([(x, y + 1.2) for x, y in rail]), 0.5)
    def rail_y(x):
        for (x0, y0), (x1, y1) in zip(rail, rail[1:]):
            if x0 <= x <= x1:
                return y0 + (y1 - y0) * (x - x0) / (x1 - x0)
        return rail[-1][1]
    pg.ink("".join(P([(x, rail_y(x) - 1.2), (x, rail_y(x) + 2.4)]) for x in range(8, 212, 5)), 0.35)
    for i in range(3):
        x = 120 + i * 17
        y = rail_y(x + 7.5) - 0.4
        pg.wash(rect(x, y - 7, 15, 6.4), C["red"] if i == 0 else C["white"], 0.85)
        pg.ink(rect(x, y - 7, 15, 6.4) + rect(x + 3, y - 5.6, 9, 2), 0.5)
    # village: half-timbered houses and the church spire
    vx, vy = 24, 121
    for i, (x, w, h) in enumerate(((vx, 12, 8), (vx + 14, 11, 9), (vx + 27, 13, 8), (vx + 52, 12, 9), (vx + 66, 11, 8))):
        roof = [(x - 1, vy - h), (x + w / 2, vy - h - 7), (x + w + 1, vy - h)]
        pg.paper(rect(x, vy - h, w, h) + P(roof, True))
        pg.wash(rect(x, vy - h, w, h), C["white"], 0.9, edge=0.4)
        pg.wash(P(roof, True), C["bsienna"], 0.8, edge=0)
        pg.ink(rect(x, vy - h, w, h) + P(roof) + P([(x, vy - h), (x + w, vy)]) + P([(x + w / 2, vy - h), (x + w / 2, vy)]), 0.45)
    cx = vx + 45
    pg.paper(rect(cx - 4, vy - 22, 8, 22) + P([(cx - 4.5, vy - 22), (cx, vy - 38), (cx + 4.5, vy - 22)], True))
    pg.wash(rect(cx - 4, vy - 22, 8, 22), C["white"], 0.9, edge=0.4)
    pg.wash(P([(cx - 4.5, vy - 22), (cx, vy - 38), (cx + 4.5, vy - 22)], True), C["teal"], 0.8, edge=0)
    pg.ink(rect(cx - 4, vy - 22, 8, 22) + P([(cx - 4.5, vy - 22), (cx, vy - 38), (cx + 4.5, vy - 22)]) +
           P([(cx, vy - 38), (cx, vy - 42)]) + P([(cx - 1.5, vy - 40.5), (cx + 1.5, vy - 40.5)]) +
           f"M{cx - 1.6} {vy - 12}v-3a1.6 1.6 0 0 1 3.2 0v3", 0.55)
    # Germany, and the free download split into four layers
    m = Equi(5.5, 15.5, 47, 55.2, 232, 10, 42)
    land = m.land(tol=0.8, only={"DEU"})
    pg.land(land, C["sapl"], 0.8)
    pg.ink(land, 0.6)
    pg.arrow([(258, 62), (251, 70)], RED_PENCIL, 0.9, 2.4, smooth=False)
    pg.arrow([(262, 62), (290, 70)], RED_PENCIL, 0.9, 2.4, smooth=False)
    cards = [(232, 72), (276, 72), (232, 122), (276, 122)]
    for k, (bx, by) in enumerate(cards):
        with pg.inset_box(bx, by, 38, 40):
            pg.wash(rect(bx, by, 38, 40), C["white"], 0.5, edge=0)
            if k == 0:
                pg.ink(S([(bx, by + 30), (bx + 20, by + 18), (bx + 38, by + 14)]) + S([(bx + 8, by), (bx + 16, by + 20), (bx + 12, by + 40)]),
                       1.2, C["ochre"])
            elif k == 1:
                tr = S([(bx, by + 28), (bx + 20, by + 20), (bx + 38, by + 16)])
                pg.ink(tr, 1.2, INK)
                pg.ink(tr, 1.0, C["white"], 1.0, "2.2 2.2")
            elif k == 2:
                rng = random.Random(7)
                for i in range(9):
                    x, y = bx + 4 + (i % 3) * 11 + rng.uniform(-1, 1), by + 5 + (i // 3) * 11 + rng.uniform(-1, 1)
                    pg.wash(rect(x, y, rng.uniform(5, 7), rng.uniform(4, 7)), C["slate"], 0.8, edge=0.5)
            else:
                for x, y in ((bx + 12, by + 18), (bx + 26, by + 30)):
                    pg.wash(rect(x - 4, y - 4, 9, 6) + f"M{x - 4} {y - 4}l2-6 2 6z", C["crimson"], 0.8)
                    pg.ink(rect(x - 4, y - 4, 9, 6) + f"M{x - 4} {y - 4}l2-6 2 6z", 0.5)
    pg.leader((cx, vy - 30), (276, 132), bend=0.1)
    pg.leader((60, 100), (232, 86), bend=-0.04)
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
