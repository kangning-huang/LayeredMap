"""Natural hazards: earthquakes, fires, tornadoes."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, P, S, circ, ell, ell_pts, rect, scallop,
                  lerp, lerp2, mix)


# ------------------------------------------------------------------ earthquakes
def _seismogram(pg: Page, x, y, w, h):
    """P wave, then the larger S wave, then the largest surface waves, decaying."""
    rng = random.Random(3)
    mid = y + h / 2
    pts = []
    tp, ts, tl = 0.18, 0.40, 0.58
    for i in range(240):
        t = i / 239
        a = 0.25
        if t > tp:
            a += 2.2 * math.exp(-(t - tp) * 18)
        if t > ts:
            a += 5.0 * math.exp(-(t - ts) * 12)
        if t > tl:
            a += 7.5 * math.exp(-(t - tl) * 6.5)
        pts.append((x + 2 + t * (w - 4), mid + rng.uniform(-1, 1) * a * h / 20))
    with pg.inset_box(x, y, w, h):
        pg.ink("".join(P([(x + 4 + i * (w - 8) / 8, y + h - 2.5), (x + 4 + i * (w - 8) / 8, y + h - 5)])
                       for i in range(9)), 0.35, INK2)
        pg.ink(P([(x + 2, y + h - 2.5), (x + w - 2, y + h - 2.5)]), 0.35, INK2)
        pg.ink(P(pts), 0.45)
    pg.callout(x + 2 + tp * (w - 4), y - 5, 1)
    pg.callout(x + 2 + ts * (w - 4), y - 5, 2)
    pg.callout(x + 2 + tl * (w - 4), y - 5, 3)


def earthquakes():
    """A strike-slip fault rupture crossing a field: the fresh scarp runs away from us, and the
    fence that crossed it has been torn apart and offset along the fault (right-lateral: the far
    block has moved to the right as seen from either side)."""
    pg = Page(1)
    # a low strip of hills, then the field as a firm vignette
    hills = [(10, 52), (40, 45), (72, 49), (104, 40), (140, 47), (176, 40), (206, 48)]
    pg.wash(S(hills + [(206, 56), (10, 56)], True), C["sky"], 0.7, edge=0)
    pg.ink(S(hills), 0.7, INK2)
    pg.vignette(6, 54, 210, 172, C["sapl"], 0.8, 4, seed=1)
    # the fault: from the near left corner to the hills; unit vector along it, pointing away
    fa, fb = (52, 176), (146, 54)
    L = math.hypot(fb[0] - fa[0], fb[1] - fa[1])
    ux, uy = (fb[0] - fa[0]) / L, (fb[1] - fa[1]) / L
    rng = random.Random(11)
    k = 16
    trace = [lerp2(fa, fb, i / k) for i in range(k + 1)]
    trace = [(x + rng.uniform(-1.2, 1.2) * (1 - i / k), y) for i, (x, y) in enumerate(trace)]
    # scarp face: a shadowed band on the right of the trace, wider near us (perspective)
    face = [(x + 7 * (1 - i / k) + 1.2, y + 1.5 * (1 - i / k)) for i, (x, y) in enumerate(trace)]
    pg.wash(P(trace + face[::-1], True), C["umber"], 0.95, edge=0)
    pg.hatch(trace + face[::-1], 20, 1.3, 0.35, INK, 0.85)
    pg.ink(P(trace), 1.3)
    pg.ink(P(face), 0.6, INK2)
    # short oblique cracks beside the trace
    for t in (0.1, 0.22, 0.36, 0.62, 0.8):
        x, y = lerp2(fa, fb, t)
        s_ = 1.2 - t
        pg.ink(P([(x + 9 * s_, y + 1), (x + 17 * s_, y - 5 * s_)]) + P([(x - 2 * s_, y - 1), (x - 9 * s_, y - 6 * s_)]), 0.45)

    # the fence: one straight line across the field before the quake, now broken at the fault
    def fence(p0, p1, h0, h1, posts):
        pts = [(lerp(p0[0], p1[0], i / posts), lerp(p0[1], p1[1], i / posts), lerp(h0, h1, i / posts))
               for i in range(posts + 1)]
        pg.wash("".join(rect(x - 1.2, y - hh, 2.4, hh) for x, y, hh in pts), C["umberl"], 0.95, edge=0)
        pg.ink("".join(P([(x, y), (x, y - hh)]) for x, y, hh in pts), 1.15)
        for f in (0.45, 0.85):
            pg.ink(P([(x, y - hh * f) for x, y, hh in pts]), 0.6)
    c = lerp2(fa, fb, 0.48)                       # where the fence crossed the fault
    left_end = (8, c[1] - 14)
    fence(left_end, (c[0] - 2, c[1]), 14, 16, 5)
    off = 18                                      # the near (east) block moved toward us
    c2 = (c[0] - ux * off + 13, c[1] - uy * off)
    fence(c2, (210, c2[1] + 18), 17, 20, 5)
    # dotted ghost: where the fence would continue without the offset
    slope = 14 / (c[0] - 10)
    for f in (0.45, 0.85):
        pg.ink(P([(c[0] + 4, c[1] - 16 * f), (c[0] + 40, c[1] + 36 * slope - 16 * f)]), 0.7, INK2, 0.9, "1.2 1.8")
    # relative motion on either side of the fault
    ax, ay = lerp2(fa, fb, 0.22)
    pg.arrow([(ax - 16, ay), (ax - 16 + ux * 18, ay + uy * 18)], RED_PENCIL, 1.3, 3.6, smooth=False)
    bx, by = lerp2(fa, fb, 0.3)
    pg.arrow([(bx + 20, by + 2), (bx + 20 - ux * 18, by + 2 - uy * 18)], RED_PENCIL, 1.3, 3.6, smooth=False)
    pg.grass(14, 200, 170, 18, 3.6, seed=1)
    # epicentre rings on the trace and the record from it
    ex, ey = lerp2(fa, fb, 0.84)
    for r in (3, 6, 9.5):
        pg.ink(ell(ex, ey, r * 1.4, r * 0.55), 0.6, RED_PENCIL, 0.9 - r / 20)
    pg.dot(ex, ey, 1.4, RED_PENCIL)
    pg.leader((ex + 2, ey - 1), (230, 34), dot_a=False)
    _seismogram(pg, 230, 18, 82, 36)
    # many small quakes, few large: circles sized by magnitude along a plate boundary
    with pg.inset_circle(271, 124, 38):
        pg.wash(circ(271, 124, 38), C["skyp"], 0.5, edge=0)
        bnd = [(236, 108), (252, 116), (266, 120), (280, 130), (296, 136), (310, 146)]
        pg.ink(S(bnd), 0.6, INK2, 0.8, "3 2")
        rng = random.Random(5)
        q = []
        for i in range(46):
            t = rng.random()
            seg = int(t * (len(bnd) - 1))
            u = t * (len(bnd) - 1) - seg
            x, y = lerp2(bnd[seg], bnd[seg + 1], u)
            mag = rng.paretovariate(1.6)
            q.append((x + rng.gauss(0, 9), y + rng.gauss(0, 9), min(0.6 + mag * 0.8, 6.5)))
        q.sort(key=lambda t: -t[2])
        for x, y, r in q:
            c_ = C["red"] if r > 3.5 else (C["orange"] if r > 1.8 else C["ochre"])
            pg.wash(circ(x, y, r), c_, 0.6, edge=0.5)
        pg.ink("".join(circ(x, y, r) for x, y, r in q if r > 1.8), 0.4)
    return pg.svg()


# ------------------------------------------------------------------ tornadoes (classic supercell)
def tornadoes():
    """A classic supercell seen from the south-east, moving to the right (north-east): the anvil
    streams downwind, the rain and hail core falls under the forward flank (right), and the
    tornado hangs from a lowered wall cloud under the rain-free base at the rear flank (left),
    with a gap of clearer air between the two."""
    pg = Page(3)
    gy = 158
    pg.ground(4, 226, gy + 4, C["sapl"], 7, 0.75)
    pg.ink(S([(4, gy), (60, gy + 1), (120, gy - 0.5), (226, gy + 0.5)]), 0.7, INK2)
    base_y = 96
    # sunlit updraft tower and the anvil: one silhouette
    tower = [(38, base_y), (34, 84), (36, 72), (42, 62), (48, 52), (56, 43), (64, 35), (72, 28), (86, 21), (98, 17),
             (101, 10), (108, 6), (116, 9), (120, 15), (160, 14), (206, 15), (236, 18), (238, 22), (214, 26), (192, 31),
             (184, 40), (186, 60), (190, 78), (194, base_y - 4)]
    pg.wash(S(tower, True, 0.4), C["skyp"], 0.7, edge=0)
    # shadowed underside of the anvil and the forward flank: darker values
    pg.wash(S([(122, 22), (190, 22), (232, 20), (214, 26), (192, 31), (184, 40), (186, 60), (190, 78), (194, base_y - 4),
               (130, base_y), (126, 70), (128, 40)], True), C["slate"], 0.7, edge=0)
    pg.wash(S([(38, base_y), (34, 84), (36, 72), (42, 62), (52, 64), (62, 80), (78, 92)], True), C["sky"], 0.6, edge=0)
    # rain-free base (dark) and the lowered, rotating wall cloud under the updraft
    pg.wash(P([(36, base_y - 2), (124, base_y - 2), (122, base_y + 3), (38, base_y + 3)], True), C["payne"], 0.85, edge=0)
    wall = [(62, base_y), (64, base_y + 9), (72, base_y + 15), (92, base_y + 16), (106, base_y + 11), (112, base_y)]
    pg.wash(S(wall, True), "#3b3f47", 0.85)
    # one funnel, dark, from the wall cloud to the ground, with its debris cloud
    fl = [(81, base_y + 15), (81, 120), (82, 136), (84, 150), (85, gy - 3)]
    fr = [(95, base_y + 15), (92, 120), (89, 136), (88, 150), (87, gy - 3)]
    pg.wash(S(fl + fr[::-1], True), "#3b3f47", 0.92)
    debris = [(72, gy), (74, gy - 8), (80, gy - 12), (92, gy - 11), (99, gy - 6), (101, gy)]
    pg.wash(S(debris, True), C["umber"], 0.75)
    # the rain and hail shaft under the forward flank, starting well right of the funnel
    rain = [(138, base_y), (160, base_y - 1), (190, base_y - 3), (196, 110), (206, 130), (204, gy), (176, gy + 1),
            (150, gy), (142, gy), (146, 136), (136, 116)]
    gid = pg.lingrad([(0, "#3b3f47", 0.9), (1, C["slate"], 0.55)])
    pg.wash_grad(S(rain, True, 0.25), gid)
    rr = random.Random(9)
    pg.ink("".join(P([(x, base_y + rr.uniform(1, 6)), (x - 6, gy - 3 - rr.uniform(0, 10))]) for x in range(146, 198, 4)),
           0.35, PAPER, 0.6)
    # ink: crisp outline for the silhouette, softer line under the anvil
    pg.ink(scallop([(38, base_y), (34, 84), (36, 72), (42, 62), (48, 52), (56, 43), (64, 35), (72, 28)], 0.32), 1.0)
    pg.ink(S(tower[7:19], False, 0.4), 1.0)
    pg.ink(S(tower[18:], False, 0.4), 0.55, INK2, 0.8)
    pg.ink(P([(36, base_y - 2), (124, base_y - 2)]), 0.9)
    pg.ink(S(wall), 0.9)
    pg.ink(S(fl) + S(fr), 1.0)
    for y in (110, 120, 130, 140, 150):
        pg.ink(S([(82, y), (86, y + 1.4), (92 - (y - 110) * .1, y)]), 0.35, C["greyl"])
    pg.ink(S(debris), 0.6)
    pg.ink("".join(f"M{x} {y}q3 4.5 6 0" for x, y in [(150, 26), (157, 26.5), (164, 27), (171, 27.5), (178, 28)]),
           0.45, INK2)    # mammatus under the anvil
    # airflow: warm inflow and updraft (red), downdrafts (blue)
    pg.arrow([(130, 152), (114, 136), (106, 116), (106, 90), (102, 60), (106, 30), (112, 18), (150, 18), (200, 19)],
             RED_PENCIL, 1.1, 3.6)
    pg.arrow([(24, 64), (24, 100), (36, 134), (60, 150)], BLUE_ARROW, 1.1, 3.4)       # rear-flank downdraft
    pg.arrow([(214, 60), (216, 100), (216, 140)], BLUE_ARROW, 1.1, 3.4)              # forward-flank downdraft
    rot = ell_pts(88, 76, 22, 5.5, 24, math.radians(-60), math.radians(-350))
    pg.arrow(rot, RED_PENCIL, 0.8, 2.8)
    house = P([(26, gy), (26, gy - 5), (30, gy - 8.5), (34, gy - 5), (34, gy)], True)
    pg.wash(house, C["bsienna"], 0.85, edge=0)
    pg.ink(house, 0.55)
    # radar view: the hook echo curls round the rear (south-west) flank, where the tornado is
    with pg.inset_circle(274, 124, 36):
        pg.wash(circ(274, 124, 36), C["skyp"], 0.4, edge=0)
        blob = [(258, 112), (268, 100), (284, 94), (300, 96), (306, 108), (300, 120), (288, 128), (278, 134),
                (268, 142), (256, 146), (250, 140), (256, 134), (264, 132), (262, 124)]
        pg.wash(S(blob, True), C["sap"], 0.7)
        pg.wash(S([(272, 108), (284, 102), (296, 106), (294, 116), (282, 122), (272, 120)], True), C["ochre"], 0.8,
                edge=0)
        pg.wash(S([(280, 108), (290, 106), (290, 113), (282, 116)], True), C["red"], 0.85, edge=0)
        pg.ink(S(blob, True), 0.6)
        pg.dot(257, 139, 1.6, INK)
        pg.arrow([(250, 108), (262, 96)], INK, 0.7, 2.4, smooth=False)
    pg.leader((88, 132), (239, 128), dot_a=True, bend=0.05)
    return pg.svg()


# ------------------------------------------------------------------ fires
def _flame(pg: Page, x, y, h, lean, seed):
    """One flame tongue: base at (x, y), leaning downwind by lean (fraction of h)."""
    rng = random.Random(seed)
    w = h * 0.32
    tip = (x + lean * h, y - h)
    left = [(x - w / 2, y), (x - w * 0.45 + lean * h * 0.3, y - h * 0.4), (x - w * 0.1 + lean * h * 0.7, y - h * 0.75), tip]
    right = [tip, (x + w * 0.35 + lean * h * 0.75, y - h * 0.62), (x + w * 0.55 + lean * h * 0.35, y - h * 0.3), (x + w / 2, y)]
    pts = left + right[1:]
    return [(px + rng.uniform(-.4, .4), py) for px, py in pts]


def fires():
    pg = Page(2)
    gy = 128
    # unburnt grass downwind (right), burnt ground upwind (left)
    pg.vignette(6, gy - 6, 210, 170, C["ochrel"], 0.55, 4, seed=1)
    burnt = [(6, gy - 4), (60, gy - 8), (96, gy - 6), (104, gy + 6), (98, 150), (106, 170), (6, 170)]
    pg.wash(S(burnt, True, 0.3), C["grey"], 0.6, edge=0)
    pg.wash(S([(14, 132), (50, 130), (86, 134), (96, 150), (92, 164), (14, 164)], True), "#3b3f47", 0.45, edge=0)
    rng = random.Random(4)
    stubs = "".join(P([(x, y), (x + rng.uniform(-1, 1), y - rng.uniform(2, 5))])
                    for x, y in [(rng.uniform(10, 92), rng.uniform(gy, 166)) for _ in range(26)])
    pg.ink(stubs, 0.45, INK)
    pg.grass(112, 206, 150, 18, 4.2, seed=2, w=0.5)
    pg.grass(118, 204, 166, 16, 4.6, seed=3, w=0.5)
    # trees downwind
    for tx, th in ((176, 34), (196, 26)):
        crown = [(tx - 9, gy - th * 0.45), (tx - 7, gy - th * 0.8), (tx, gy - th), (tx + 7, gy - th * 0.8),
                 (tx + 9, gy - th * 0.45), (tx, gy - th * 0.3)]
        pg.wash(S(crown, True), C["hooker"], 0.6)
        pg.ink(S(crown, True), 0.6)
        pg.ink(P([(tx, gy - th * 0.3), (tx, gy + 2)]), 0.8)
    # smoke column: rises from the front and bends downwind (right), spreading as it goes
    smoke = [(86, gy - 12), (88, 98), (98, 72), (116, 50), (142, 32), (176, 20), (212, 14), (214, 24), (186, 32),
             (160, 44), (140, 62), (124, 86), (114, gy - 10)]
    pg.wash(S(smoke, True, 0.45), C["greyl"], 0.75)
    pg.wash(S([(214, 24), (186, 32), (160, 44), (140, 62), (124, 86), (114, gy - 10), (106, 100), (118, 70), (146, 46),
               (180, 30)], True), C["slate"], 0.4, edge=0)
    pg.ink(scallop([(88, 98), (90, 86), (98, 72), (106, 60), (116, 50), (128, 40), (142, 32), (158, 25), (176, 20),
                    (194, 16), (212, 14)], 0.3), 0.6, INK2)
    # flame front along the boundary
    flames = []
    for i, (fx, fy, fh) in enumerate([(84, gy - 4, 16), (92, gy - 1, 22), (99, gy + 3, 18), (103, gy + 9, 24),
                                      (102, gy + 17, 17), (104, gy + 26, 20), (108, gy + 34, 15)]):
        flames.append(_flame(pg, fx, fy, fh, 0.35, i))
    for f in flames:
        pg.wash(S(f, True, 0.4), C["orange"], 0.85)
    for f in flames:
        inner = [(lerp(p[0], f[0][0] * .5 + f[-1][0] * .5, 0.35), lerp(p[1], f[0][1], 0.35)) for p in f]
        pg.wash(S(inner, True, 0.4), C["lemon"], 0.9, edge=0)
    for f in flames:
        pg.ink(S(f, False, 0.4), 0.55, C["bsienna"])
    # wind (blue) and spread direction (red)
    pg.arrow([(14, 52), (34, 50), (54, 52)], BLUE_ARROW, 1.0, 3.2)
    pg.arrow([(14, 62), (30, 61), (46, 62)], BLUE_ARROW, 1.0, 3.2)
    pg.arrow([(118, 172), (146, 170), (168, 172)], RED_PENCIL, 1.1, 3.4)
    # the satellite's view: hot pixels on a grid
    sx, sy = 254, 14
    pg.wash(rect(sx - 3, sy - 3, 6, 6), C["greyl"], 0.9)
    pg.ink(rect(sx - 3, sy - 3, 6, 6), 0.6)
    pg.wash(rect(sx - 15, sy - 2, 10, 4) + rect(sx + 5, sy - 2, 10, 4), C["ultra"], 0.85)
    pg.ink(rect(sx - 15, sy - 2, 10, 4) + rect(sx + 5, sy - 2, 10, 4) + P([(sx - 5, sy), (sx - 3, sy)]) +
           P([(sx + 3, sy), (sx + 5, sy)]), 0.5)
    gx, gy0, cs = 236, 34, 9.5
    pg.ink(P([(sx - 2, sy + 4), (gx, gy0)]) + P([(sx + 2, sy + 4), (gx + cs * 8, gy0)]), 0.4, INK2, 0.7, "1.5 1.5")
    with pg.inset_box(gx, gy0, cs * 8, cs * 6):
        pg.wash(rect(gx, gy0, cs * 8, cs * 6), C["sapl"], 0.45, edge=0)
        hot = {(2, 0): 1, (3, 1): 2, (3, 2): 3, (4, 2): 2, (4, 3): 3, (5, 4): 2, (5, 5): 1, (6, 4): 1, (2, 1): 1}
        for (i, j), k in hot.items():
            c = {1: C["lemon"], 2: C["orange"], 3: C["red"]}[k]
            pg.wash(rect(gx + i * cs + .6, gy0 + j * cs + .6, cs - 1.2, cs - 1.2), c, 0.9, edge=0)
        pg.ink("".join(P([(gx + i * cs, gy0), (gx + i * cs, gy0 + 6 * cs)]) for i in range(1, 8)) +
               "".join(P([(gx, gy0 + j * cs), (gx + 8 * cs, gy0 + j * cs)]) for j in range(1, 6)), 0.3, INK2)
    pg.leader((103, gy + 6), (gx, gy0 + 3 * cs), bend=-0.08)
    # the same fire line by night
    with pg.inset_circle(274, 140, 30):
        pg.wash(circ(274, 140, 30), C["night"], 0.85, edge=0)
        pg.ink(S([(244, 150), (262, 146), (280, 148), (304, 144)]), 0.5, C["sky"], 0.6)
        for x, y, r in ((256, 150, 2.2), (264, 147, 2.8), (271, 146, 2.0), (279, 148, 3.0), (287, 147, 2.2),
                        (294, 146, 1.6)):
            pg.wash(circ(x, y, r * 1.8), C["orange"], 0.45, edge=0)
            pg.raw(f'<circle cx="{x}" cy="{y}" r="{r * .6:.1f}" fill="{C["lemon"]}"/>')
        pg.raw(f'<circle cx="288" cy="124" r="3.4" fill="{C["white"]}"/><circle cx="289.6" cy="123" r="3" fill="{C["night"]}" fill-opacity=".9"/>')
    return pg.svg()
