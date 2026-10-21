"""Living things: storks, street trees, species sightings, forest loss."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, Page, Equi, P, S, circ, ell, ell_pts, rect,
                  lerp, lerp2, mix, jitter)


# ------------------------------------------------------------------ white stork
def _stork_standing(pg: Page):
    # construction: posture axis and body oval, in blue pencil
    pg.construct(S(ell_pts(116, 84, 38, 15, 28, rot=0.12), True))
    pg.construct(P([(62, 34), (78, 70), (156, 92)]))
    pg.construct(P([(112, 99), (112, 159)]), op=0.4)

    body = [(84, 72), (100, 67), (124, 69), (142, 75), (156, 84), (150, 89), (134, 94), (116, 99),
            (99, 99), (85, 94), (79, 85)]
    neck = [(80, 87), (77, 76), (76, 64), (72, 53), (65, 45), (60, 44), (55, 42), (54, 37), (57, 32),
            (63, 29), (69, 31), (73, 37), (79, 47), (86, 58), (93, 69)]
    # washes: white bird = shadow side only (cool grey), black flight feathers, red bill and legs
    pg.wash(S([(86, 92), (100, 98), (118, 98), (136, 93), (124, 88), (104, 90)], True), C["sky"], 0.5, edge=0)
    pg.wash(S([(78, 84), (77, 72), (82, 74), (84, 88)], True), C["skyp"], 0.8, edge=0)
    wing = [(104, 82), (122, 80), (140, 82), (152, 86), (166, 92), (150, 93), (132, 96), (114, 97), (101, 93)]
    pg.wash(S(wing, True), "#3b3f47", 0.88)
    bill = [(55, 36), (31, 51), (28, 53), (32, 52.5), (56, 41.5)]
    pg.wash(P(bill, True), C["red"], 0.85)
    # legs: front leg straight, hind leg a little behind
    legs = []
    for (hx, hy), (jx, jy), (fx, fy) in (((110, 98), (111, 128), (107, 158)),
                                         ((120, 97), (122, 127), (125, 157))):
        legs.append(((hx, hy), (jx, jy), (fx, fy)))
        pg.stroke_wash(P([(hx, hy + 5), (jx, jy), (fx, fy)]), C["red"], 2.3, 0.8)
    # ink: contours
    pg.ink(S(body[:5], False), 0.95)
    pg.ink(S(body[5:] + [body[0]], False), 0.95)
    pg.ink(S(neck[:6]), 0.9)
    pg.ink(S(neck[6:]), 0.9)
    pg.ink(S([(55, 42), (54, 37), (57, 32), (63, 29), (69, 31)]), 0.9)
    pg.ink(P(bill[:2]) + P(bill[3:][::-1] + [bill[2]]), 0.8)
    pg.ink(P([(55, 39), (40, 46.5)]), 0.35)            # gape line
    # folded wing: scalloped coverts edge over the black flight feathers, primaries crossing the tail
    pg.ink(S(wing, True), 0.75)
    sc = []
    for i in range(6):
        x = 106 + i * 8
        sc.append(f"M{x} {81 + i * .8}q4 3 8 {.6 + .2 * i}")
    pg.ink("".join(sc), 0.45)
    pg.hatch([(124, 88), (150, 88), (162, 92), (132, 95)], -20, 1.3, 0.35, INK, 0.7)
    for k in range(3):
        pg.ink(P([(132 + k * 6, 90 + k * .6), (163 - k * 3, 92 - k * .4)]), 0.35)
    # eye with black bare skin, white glint left as paper
    pg.ink_fill(ell(60, 35, 2.4, 1.8), INK, 0.9)
    pg.raw('<circle cx="59.4" cy="34.4" r=".55" fill="#fbf8f0"/>')
    # lower neck: the long hackle feathers, a few pen ticks
    pg.ink("".join(P([(76.5 + i * 1.2, 72 + i * 2.6), (74.6 + i * 1.2, 75.5 + i * 2.6)]) for i in range(4)), 0.3, INK2, 0.7)
    # legs ink and the backwards-bending ankle (intertarsal joint)
    for (hx, hy), (jx, jy), (fx, fy) in legs:
        pg.ink(P([(hx - 1.1, hy + 4), (jx - 1.3, jy), (fx - 1.1, fy)]), 0.6)
        pg.ink(P([(hx + 1.1, hy + 4), (jx + 1.2, jy), (fx + 1.1, fy)]), 0.6)
        # feathered thigh
        pg.ink(S([(hx - 3, hy - 1), (hx - 1.6, hy + 5), (hx + 1.6, hy + 5), (hx + 3, hy - 1)]), 0.6)
        # three forward toes and a short hind toe
        pg.ink(P([(fx, fy), (fx - 9, fy + 1.6)]) + P([(fx, fy), (fx - 7, fy + 2.6)]) +
               P([(fx, fy), (fx - 4, fy + 2.8)]) + P([(fx, fy), (fx + 3.5, fy + 1)]), 0.75)


def _stork_flight(pg: Page, cx, cy, s=1.0):
    """A soaring stork seen from below: neck out, legs trailing, broad wings with black flight
    feathers along the trailing edge and slotted (fingered) primaries at the tips."""
    def T(pts):
        return [(cx + x * s, cy + y * s) for x, y in pts]
    for sgn in (-1, 1):
        wing = T([(-4, 1.5 * sgn), (-6, 10 * sgn), (-8, 19 * sgn), (-9, 26 * sgn), (-5, 30 * sgn), (1, 31 * sgn),
                  (7, 29 * sgn), (10, 20 * sgn), (10, 10 * sgn), (7, 2 * sgn)])
        black = T([(1, 3 * sgn), (2, 12 * sgn), (0, 21 * sgn), (-6, 25 * sgn), (-9, 26 * sgn), (-5, 30 * sgn),
                   (1, 31 * sgn), (7, 29 * sgn), (10, 20 * sgn), (10, 10 * sgn), (7, 2 * sgn)])
        pg.wash(S(black, True), "#3b3f47", 0.85, edge=0)
        pg.ink(S(wing[:4]), 0.65)
        pg.ink(S(wing[6:]), 0.65)
        # five slotted primaries ("fingers") at the wing tip
        for k in range(5):
            a = T([(-8 + k * 3, (25.5 + k * .9) * sgn), (-9.5 + k * 3.2, (31 + k * .2) * sgn)])
            pg.ink(P(a), 0.55)
        pg.ink(S(T([(-1.5, 2 * sgn), (0, 12 * sgn), (-2, 21 * sgn), (-7, 25 * sgn)])), 0.35, INK2)
    body = T([(-10, 0), (-5, -3), (6, -3), (13, -1.2), (13, 1.2), (6, 3), (-5, 3)])
    pg.wash(S(T([(-4, 1.5), (8, 2.6), (12, 1), (0, 2.8)]), True), C["sky"], 0.6, edge=0)
    pg.ink(S(body, True), 0.65)
    pg.ink(P(T([(-10, -.9), (-18, -.6)])) + P(T([(-10, .9), (-18, .7)])), 0.6)     # neck stretched forward
    bill = T([(-18, -.5), (-27, .6), (-18, .9)])
    pg.wash(P(bill, True), C["red"], 0.95, edge=0)
    pg.ink(P(bill), 0.45)
    for dy in (-1, 1):                                                       # legs trail past the tail
        pg.stroke_wash(P(T([(11, dy * 1.1), (24, dy * 1.5)])), C["red"], 1.4 * s, 0.9)
        pg.ink(P(T([(11, dy * 1.1), (24, dy * 1.5)])), 0.35)


def _flyway_map(pg: Page, x, y, w):
    m = Equi(-18, 42, -2, 58, x, y, w)
    h = m.h
    with pg.inset_box(x, y, w, h):
        pg.wash(rect(x, y, w, h), C["sky"], 0.45, edge=0)
        pg.land(m.land(tol=1.4), C["sand"], 0.9)
        # the two flyways around the Mediterranean, drawn as chains of GPS fixes
        west = [(10, 52.5), (7, 49.5), (3, 46.5), (-1, 42.5), (-4.5, 38.5), (-5.6, 35.8), (-6.5, 32),
                (-7.5, 27), (-6, 21), (-3, 16), (0, 13)]
        east = [(12, 52), (17, 49), (21, 46), (25.5, 43), (29, 41), (32, 38.5), (35.5, 35.5), (35, 31.5),
                (32.8, 28), (32.5, 23), (33, 18), (34, 13), (35.5, 8), (36, 3)]
        for route in (west, east):
            pts = [m(*p) for p in route]
            pg.arrow(pts, RED_PENCIL, 0.9, 3.0)
            for p in pts[1:-1]:
                pg.dot(p[0], p[1], 0.75, INK)
        bx, by = m(11, 52.3)
        pg.ink(circ(bx, by, 2.6), 0.6)
    return m


def _gps_tag(pg: Page, cx, cy, r):
    """Close-up of the bird's back: a solar GPS logger on a harness, antenna trailing behind."""
    with pg.inset_circle(cx, cy, r):
        back = [(cx - r - 2, cy + 2), (cx - r * .5, cy - 4), (cx, cy - 5.5), (cx + r * .5, cy - 4), (cx + r + 2, cy)]
        under = back + [(cx + r + 2, cy + r + 2), (cx - r - 2, cy + r + 2)]
        pg.wash(S(under, True, 0.3), C["skyp"], 0.55, edge=0)
        # black flight feathers below the white coverts
        wing = [(cx - r - 2, cy + 10), (cx - r * .3, cy + 7), (cx + r * .5, cy + 7), (cx + r + 2, cy + 6),
                (cx + r + 2, cy + r + 2), (cx - r - 2, cy + r + 2)]
        pg.wash(S(wing, True, 0.3), "#3b3f47", 0.85, edge=0)
        pg.ink(S(wing[:4]), 0.6)
        pg.ink(S(back), 0.8)
        # covert feathers: small overlapping U-strokes pointing back (to the right)
        f = []
        for row in range(2):
            for i in range(6):
                fx = cx - r + 5 + i * 6.4 + row * 3
                fy = cy - 1 + row * 4
                f.append(f"M{fx:.1f} {fy:.1f}q3 .4 3.4 2.4")
        pg.ink("".join(f), 0.35, INK2)
        lx, ly = cx - 9, cy - 12.6
        pg.ink(P([(lx + 3, ly + 8), (cx - 7, cy + r)]) + P([(lx + 15, ly + 8), (cx + 11, cy + r)]), 0.6)  # harness
        pg.wash(rect(lx, ly, 18, 8), C["ochrel"], 0.85)
        pg.wash(rect(lx + 2, ly + 1.6, 14, 4.6), C["ultra"], 0.85, edge=0)
        pg.ink(rect(lx, ly, 18, 8), 0.7)
        pg.ink(rect(lx + 2, ly + 1.6, 14, 4.6) + P([(lx + 6.7, ly + 1.6), (lx + 6.7, ly + 6.2)]) +
               P([(lx + 11.3, ly + 1.6), (lx + 11.3, ly + 6.2)]), 0.35)
        pg.ink(S([(lx + 18, ly + 4), (lx + 26, ly + 4.5), (lx + 36, ly + 8)]), 0.55)        # antenna


def storks():
    pg = Page(24)
    pg.ground(30, 180, 160, C["sapl"], 7, 0.55)
    pg.grass(70, 160, 160, 14, 3.4)
    _stork_standing(pg)
    _stork_flight(pg, 190, 34, 1.0)
    # the logger on the bird's back and its close-up
    pg.leader((124, 68), (171, 109))
    _gps_tag(pg, 188, 124, 22)
    m = _flyway_map(pg, 230, 70, 82)
    # a scale bar for the bird
    pg.scalebar(20, 170, 40, 4)
    return pg.svg()


# ------------------------------------------------------------------ street trees (London plane, ginkgo)
def _plane_leaf(pg: Page, cx, cy, s):
    # palmate, five-lobed, toothed margin, petiole at the base
    pts = []
    lobes = [(-90, 1.0), (-35, 0.9), (20, 0.62), (160, 0.62), (215, 0.9)]
    angs = [a for a, _ in lobes]
    for i, (a, L) in enumerate(lobes):
        a0 = math.radians(a)
        pts.append((cx + math.cos(a0) * s * L, cy + math.sin(a0) * s * L))
        b = math.radians(a + ((angs[(i + 1) % 5] - a) % 360) / 2)
        pts.append((cx + math.cos(b) * s * 0.6, cy + math.sin(b) * s * 0.6))
    # a few coarse teeth between the lobe tips and the sinuses
    full = []
    for i in range(len(pts)):
        p, q = pts[i], pts[(i + 1) % len(pts)]
        full.append(p)
        mx, my = lerp2(p, q, 0.5)
        dx, dy = q[0] - p[0], q[1] - p[1]
        L = math.hypot(dx, dy) or 1
        full.append((mx + dy / L * 1.2, my - dx / L * 1.2))
    pg.wash(S(full, True, 0.35), C["sap"], 0.75)
    pg.ink(S(full, True, 0.35), 0.7)
    for a, L in lobes:
        a0 = math.radians(a)
        pg.ink(P([(cx, cy + s * .1), (cx + math.cos(a0) * s * L * 0.9, cy + math.sin(a0) * s * L * 0.9)]), 0.4)
    pg.ink(S([(cx, cy + s * .1), (cx - s * .05, cy + s * .55), (cx + s * .08, cy + s * .9)]), 0.7)


def _ginkgo_leaf(pg: Page, cx, cy, s):
    # fan-shaped, notched in the middle, fine radiating (dichotomous) veins, long petiole
    base = (cx, cy + s * 0.55)
    fan = [base]
    for i in range(13):
        a = math.radians(-155 + i * (130 / 12))
        rr = s * (0.95 if abs(i - 6) > 0 else 0.62)
        fan.append((cx + math.cos(a) * rr, cy + s * 0.05 + math.sin(a) * rr * 0.8))
    pg.wash(P(fan, True), C["lemon"], 0.8)
    pg.ink(S(fan[1:], False, 0.3) + P([fan[1], base, fan[-1]]), 0.7)
    vs = []
    for i in range(1, 12):
        a = math.radians(-152 + i * (124 / 11))
        vs.append(P([base, (cx + math.cos(a) * s * .82, cy + s * .05 + math.sin(a) * s * .65)]))
    pg.ink("".join(vs), 0.3, INK2)
    pg.ink(S([base, (cx + s * .08, cy + s * .95), (cx + s * .02, cy + s * 1.3)]), 0.7)


def street_trees():
    pg = Page(25)
    # a building facade sketched in light pencil behind (a stoop and windows)
    pg.ink(P([(6, 30), (6, 138)]) + P([(6, 30), (36, 30)]) + P([(172, 30), (200, 30)]) + P([(200, 30), (200, 138)]),
           0.4, INK2, 0.5)
    for wx, wy in ((14, 84), (14, 110), (178, 86), (178, 110)):
        pg.ink(rect(wx, wy, 12, 18), 0.4, INK2, 0.5)
    # sidewalk and curb in perspective-free elevation
    pg.vignette(4, 138, 212, 153, C["greyl"], 0.6, 2.5, seed=1)
    pg.vignette(6, 156, 208, 172, C["payne"], 0.3, 3, seed=2)
    pg.ink(P([(4, 138), (212, 138)]) + P([(6, 153), (210, 153)]) + P([(8, 156), (206, 156)]), 0.7)
    for x in range(14, 204, 26):
        pg.ink(P([(x, 138.5), (x - 4, 152.5)]), 0.3, INK2)
    # tree pit with soil
    pg.wash(rect(78, 138, 46, 5), C["umber"], 0.7, edge=0)
    pg.ink(rect(78, 138, 46, 5), 0.6)
    # trunk of a London plane: mottled, peeling bark
    trunk = [(94, 140), (96, 112), (95, 92), (90, 74), (84, 62), (92, 66), (99, 78), (103, 70), (110, 58),
             (114, 61), (108, 80), (107, 98), (108, 118), (110, 140)]
    pg.wash(P(trunk, True), C["ochrel"], 0.85)
    rng = random.Random(7)
    for _ in range(12):
        x, y = rng.uniform(96, 107), rng.uniform(80, 136)
        pg.wash(S(ell_pts(x, y, rng.uniform(1.8, 3), rng.uniform(2.5, 4.5), 7), True),
                rng.choice([C["olive"], C["grey"], C["umberl"]]), 0.7, edge=0.6)
    # crown: broad, rounded, lobed masses; light from the top left, shadow below right
    crown = [(30, 60), (34, 38), (52, 22), (76, 12), (104, 8), (132, 12), (158, 24), (176, 42), (178, 62),
             (164, 76), (138, 82), (112, 80), (100, 84), (82, 80), (56, 80), (38, 74)]
    pg.wash(S(crown, True), C["sapl"], 0.8)
    pg.wash(S([(44, 70), (70, 60), (104, 66), (140, 58), (170, 60), (162, 76), (138, 82), (100, 84), (56, 80)],
              True), C["hooker"], 0.55, edge=0)
    pg.wash(S([(66, 38), (90, 30), (112, 40), (138, 34), (150, 50), (120, 56), (86, 52)], True), C["sap"], 0.4,
            edge=0)
    pg.ink(S(crown, True, 0.55), 0.8)
    # leaf-clump scallops inside the crown
    # leaf masses: bumpy contours of the clumps on their shadowed undersides
    for pts in ([(44, 60), (54, 64), (66, 62), (78, 66), (90, 63)], [(110, 66), (122, 70), (136, 67), (150, 70),
                (164, 66)], [(62, 40), (72, 44), (84, 42), (96, 46)], [(116, 44), (128, 48), (140, 46), (152, 50)]):
        d = ""
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            d += f"M{x0:.1f} {y0:.1f}Q{(x0 + x1) / 2:.1f} {max(y0, y1) + 4:.1f} {x1:.1f} {y1:.1f}"
        pg.ink(d, 0.45, INK2)
    pg.ink(S(trunk[:5]) + S(trunk[9:]), 0.85)
    pg.ink(S(trunk[4:9]), 0.6)
    pg.hatch([(104, 96), (108, 96), (109, 138), (105, 138)], -70, 1.4, 0.35)
    # tape around the trunk at breast height, and the height of that tape
    ty = 120
    pg.wash(rect(95.5, ty - 1.4, 13, 2.8), C["ochre"], 0.9, edge=0)
    pg.ink(P([(95.5, ty - 1.4), (108.5, ty - 1.4)]) + P([(95.5, ty + 1.4), (108.5, ty + 1.4)]), 0.5)
    pg.arrow([(88, 137), (88, ty)], RED_PENCIL, 0.8, 2.6, both=True, smooth=False)
    pg.ink(P([(85, ty), (95, ty)]), 0.4, INK, 0.7, "1 1.2")
    pg.person(64, 138, 22)
    # insets: plane leaf (with its seed ball) and a ginkgo leaf
    with pg.inset_circle(268, 48, 32):
        pg.wash(circ(268, 48, 32), C["skyp"], 0.35, edge=0)
        _plane_leaf(pg, 262, 44, 20)
        pg.ink(S([(282, 22), (284, 38), (288, 54)]), 0.5)                     # seed-ball stalk
        pg.wash(circ(288, 58, 5), C["umberl"], 0.85)
        pg.ink(circ(288, 58, 5), 0.6)
        pg.ink("".join(P([(288 + 3.5 * math.cos(a), 58 + 3.5 * math.sin(a)), (288 + 5.6 * math.cos(a), 58 + 5.6 * math.sin(a))])
                       for a in [i * 0.7 for i in range(9)]), 0.3)
    with pg.inset_circle(268, 130, 32):
        pg.wash(circ(268, 130, 32), C["skyp"], 0.35, edge=0)
        _ginkgo_leaf(pg, 266, 124, 22)
    pg.leader((160, 40), (236, 46))
    pg.scalebar(14, 170, 40, 4)
    return pg.svg()


# ------------------------------------------------------------------ species (a monarch butterfly)
def _monarch(pg: Page, cx, cy, s):
    """Monarch, dorsal view: orange wings with black veins, a black border with white spots, a
    black forewing tip with pale spots; head, thorax, abdomen; clubbed antennae."""
    def T(pts, sx=1):
        return [(cx + x * s * sx, cy + y * s) for x, y in pts]
    for sx in (-1, 1):
        fw = T([(2, -6), (14, -24), (34, -34), (52, -36), (58, -30), (52, -18), (40, -6), (22, 0), (4, 0)], sx)
        hw = T([(3, 1), (20, 0), (38, 4), (46, 14), (42, 28), (28, 36), (14, 34), (5, 22), (2, 8)], sx)
        for wing in (hw, fw):
            pg.wash(S(wing, True, 0.35), C["orange"], 0.85, edge=0)
            pg.ink(S(wing, True, 0.35), 0.7 + 0.0)
        # black forewing apex and borders
        tip = T([(34, -34), (52, -36), (58, -30), (52, -18), (46, -20), (48, -28), (40, -30)], sx)
        pg.wash(S(tip, True, 0.3), "#2f2d2b", 0.85, edge=0)
        border = T([(52, -18), (40, -6), (22, 0), (22, -3), (38, -9), (49, -20)], sx)
        pg.wash(S(border, True, 0.3), "#2f2d2b", 0.85, edge=0)
        hb = T([(38, 4), (46, 14), (42, 28), (28, 36), (14, 34), (14, 31), (26, 32), (38, 25), (42, 14), (36, 7)], sx)
        pg.wash(S(hb, True, 0.3), "#2f2d2b", 0.85, edge=0)
        # veins
        v = ""
        for a, b in (((4, -3), (34, -32)), ((6, -2), (48, -24)), ((8, -1), (42, -12)), ((10, 0), (28, -3)),
                     ((4, 3), (40, 10)), ((4, 5), (42, 20)), ((4, 8), (34, 30)), ((4, 10), (22, 33)), ((4, 12), (12, 30))):
            pa, pb = T([a, b], sx)
            v += S([pa, lerp2(pa, pb, 0.5), pb])
        pg.ink(v, 0.6, "#2f2d2b")
        # white spots in the borders
        dots = ""
        for x, y in ((50, -30), (46, -32), (54, -26), (50, -22), (44, -14), (36, -7), (28, -3), (42, 20), (38, 28), (30, 33),
                     (20, 33), (44, 12)):
            px, py = T([(x, y)], sx)[0]
            dots += circ(px, py, 0.7 * s * 1.6)
        pg.raw(f'<path d="{dots}" fill="{C["white"]}"/>')
        # antenna with its club
        a0, a1 = T([(1, -8), (10, -30)], sx)
        pg.ink(S([a0, (a0[0] + (a1[0] - a0[0]) * 0.5 + sx * 1, a0[1] + (a1[1] - a0[1]) * 0.5), a1]), 0.5)
        pg.ink_fill(ell(a1[0], a1[1], 1.1 * s * 1.4, 1.6 * s * 1.4), "#2f2d2b")
    # body: head, thorax, abdomen, black with white dots
    body = T([(0, -10), (2.4, -7), (2.6, 0), (2.2, 14), (0.8, 24), (0, 26), (-0.8, 24), (-2.2, 14), (-2.6, 0), (-2.4, -7)])
    pg.ink_fill(S(body, True, 0.35), "#2f2d2b")
    pg.raw("".join(f'<circle cx="{cx + x * s:.1f}" cy="{cy + y * s:.1f}" r=".5" fill="{C["white"]}"/>'
                   for x, y in ((-1, -8), (1, -8), (-1.2, -2), (1.2, -2))))


def species():
    pg = Page(23)
    # construction axis and the wing triangle, in blue pencil
    pg.construct(P([(100, 18), (100, 160)]))
    pg.construct(P([(18, 46), (100, 82), (182, 46)]), op=0.4)
    _monarch(pg, 100, 82, 1.45)
    # wing scales, magnified: overlapping shingles in rows, orange and black
    with pg.inset_circle(266, 48, 32):
        rows = 8
        for r in range(rows):
            for i in range(9):
                x = 238 + i * 7 + (r % 2) * 3.5
                y = 22 + r * 7
                c = "#2f2d2b" if (i + r // 2) > 9 - r // 3 else C["orange"]
                sc = f"M{x:.1f} {y:.1f}h5.4v5.4a2.7 2 0 0 1 -5.4 0z"
                pg.wash(sc, c, 0.85, edge=0.6)
                pg.ink(sc, 0.3)
    pg.leader((140, 62), (236, 52), bend=-0.08)
    # where people saw them: sightings over North America, crowding in central Mexico in winter
    m = Equi(-125, -66, 14, 54, 228, 92, 84)
    with pg.inset_box(228, 92, 84, m.h):
        pg.wash(rect(228, 92, 84, m.h), C["sky"], 0.45, edge=0)
        pg.land(m.land(tol=1.2), C["sand"], 0.9)
        rng = random.Random(23)
        pts = [m(rng.uniform(-104, -70), rng.uniform(28, 48)) for _ in range(70)]
        pts += [m(-100.3 + rng.gauss(0, 0.6), 19.6 + rng.gauss(0, 0.4)) for _ in range(28)]
        pg.dots(pts, 0.75, C["red"])
        pg.arrow([m(-90, 44), m(-96, 34), m(-100, 22)], RED_PENCIL, 0.8, 2.6)
    pg.scalebar(130, 168, 50, 4)
    return pg.svg()


# ------------------------------------------------------------------ forest loss
def _conifer(pg: Page, x, gy, h, c=None):
    w = h * 0.32
    tri = [(x, gy - h), (x - w * 0.55, gy - h * 0.55), (x - w * 0.35, gy - h * 0.58), (x - w, gy - h * 0.12),
           (x + w, gy - h * 0.12), (x + w * 0.35, gy - h * 0.58), (x + w * 0.55, gy - h * 0.55)]
    pg.wash(P(tri, True), c or C["hooker"], 0.75)
    pg.ink(P(tri, True), 0.5)
    pg.ink(P([(x, gy - h * 0.12), (x, gy)]), 0.8, C["umber"])


def forest_loss():
    pg = Page(26)
    gy = 130
    pg.vignette(4, gy - 4, 214, 168, C["sapl"], 0.45, 3, seed=1)
    # standing forest on the left and behind; the far forest stands on a distant slope
    pg.wash(S([(100, 92), (150, 88), (212, 90), (212, 126), (100, 128)], True), C["sapl"], 0.4, edge=0)
    rng = random.Random(26)
    for k in range(16):
        x = 8 + k * 6.5 + rng.uniform(-2, 2)
        _conifer(pg, x, gy - 8 + (k % 3) * 3, rng.uniform(40, 60))
    for k in range(10):
        x = 120 + k * 9
        _conifer(pg, x, 92, rng.uniform(14, 20), C["sap"])
    # the clearing: bare soil, stumps, logs piled by the track
    pg.wash(S([(104, gy - 6), (212, gy - 10), (212, 168), (100, 168), (110, 150)], True), C["umberl"], 0.55, edge=0)
    stumps = ""
    for x, y in ((120, 136), (134, 144), (150, 132), (164, 148), (178, 138), (196, 146), (140, 158), (186, 160), (118, 154)):
        pg.wash(P([(x - 2.4, y), (x - 2, y - 4), (x + 2, y - 4), (x + 2.4, y)], True), C["umber"], 0.8, edge=0)
        pg.wash(ell(x, y - 4, 2, 0.8), C["ochrel"], 0.95, edge=0)
        stumps += P([(x - 2.4, y), (x - 2, y - 4)]) + P([(x + 2, y - 4), (x + 2.4, y)]) + ell(x, y - 4, 2, 0.8)
    pg.ink(stumps, 0.45)
    for i, (x, y) in enumerate(((176, 134), (182, 134), (188, 134), (179, 130.6), (185, 130.6), (182, 127.2))):
        pg.wash(P([(x - 12, y), (x + 12, y), (x + 12, y + 3.4), (x - 12, y + 3.4)], True), C["sienna"], 0.7, edge=0)
        pg.wash(ell(x + 12, y + 1.7, 1.2, 1.7), C["ochrel"], 0.95, edge=0)
        pg.ink(P([(x - 12, y), (x + 12, y)]) + P([(x - 12, y + 3.4), (x + 12, y + 3.4)]) + ell(x + 12, y + 1.7, 1.2, 1.7), 0.45)
    pg.ink(S([(104, 168), (130, 150), (170, 128), (212, 118)]), 1.1, C["umber"], 0.7, "3 2")      # skid track
    # the 30 m pixels: forest (green) and the year each pixel was lost, early (yellow) to recent (red)
    gx, gy0, cs = 232, 18, 8
    with pg.inset_box(gx, gy0, cs * 10, cs * 8):
        for i in range(10):
            for j in range(8):
                d = (i - 2) + (j * 0.4) + rng.uniform(-1.4, 1.4)
                if d < 2:
                    c = C["hooker"]
                else:
                    t = min(1, (d - 2) / 7)
                    c = mix(C["lemon"], C["crimson"], t)
                pg.raw(f'<rect x="{gx + i * cs}" y="{gy0 + j * cs}" width="{cs}" height="{cs}" fill="{c}" fill-opacity=".8"/>')
        pg.ink("".join(P([(gx + i * cs, gy0), (gx + i * cs, gy0 + 8 * cs)]) for i in range(1, 10)) +
               "".join(P([(gx, gy0 + j * cs), (gx + 10 * cs, gy0 + j * cs)]) for j in range(1, 8)), 0.25, INK2, 0.8)
    for k in range(10):
        pg.raw(f'<rect x="{gx + k * 8}" y="88" width="8" height="5" fill="{mix(C["lemon"], C["crimson"], k / 9)}"/>')
    pg.ink(rect(gx, 88, 80, 5), 0.45)
    pg.arrow([(gx, 98), (gx + 80, 98)], INK, 0.6, 2.2, smooth=False)
    pg.leader((160, 140), (gx, 60), bend=-0.08)
    # a stump, close up: its rings
    with pg.inset_circle(272, 140, 26):
        pg.wash(circ(272, 140, 26), C["sapl"], 0.4, edge=0)
        pg.wash(ell(272, 140, 18, 15), C["ochrel"], 0.9)
        pg.ink("".join(S(ell_pts(272 + k * 0.3, 140 + k * 0.2, 18 - k * 2.2, 15 - k * 1.85, 18), True) for k in range(8)), 0.35,
               C["umber"])
        pg.ink(ell(272, 140, 18, 15), 0.7)
    return pg.svg()
