"""Energy: wind turbines, nuclear reactors, solar farms, mines."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, P, S, circ, ell, ell_pts, rect, scallop,
                  lerp, lerp2, mix)


# ------------------------------------------------------------------ wind turbines
def _blade(cx, cy, L, ang, chord=0.09, fx=0.6):
    """A tapered blade from the hub outwards at angle ang in the rotor plane, as a closed outline.
    The rotor plane is turned away from us, so x is foreshortened by fx (3/4 view from upwind)."""
    ux, uy = math.cos(ang) * fx, math.sin(ang)
    vx, vy = -math.sin(ang) * fx, math.cos(ang)
    prof = [(0.06, 0.45), (0.2, 1.0), (0.45, 0.75), (0.75, 0.45), (1.0, 0.12)]
    lead = [(cx + ux * L * t + vx * L * chord * w * 0.35, cy + uy * L * t + vy * L * chord * w * 0.35) for t, w in prof]
    trail = [(cx + ux * L * t - vx * L * chord * w * 0.65, cy + uy * L * t - vy * L * chord * w * 0.65) for t, w in prof]
    return lead + trail[::-1]


def _turbine(pg: Page, x, gy, hub_h, s_blade, phase=0.0, detail=True, w0=6.0):
    hx, hy = x, gy - hub_h
    tower = [(x - w0 / 2, gy), (x - w0 * 0.3, hy + 3), (x + w0 * 0.3, hy + 3), (x + w0 / 2, gy)]
    pg.wash(P(tower, True), C["white"], 0.9, edge=0)
    pg.wash(P([(x + w0 * 0.05, gy), (x + w0 * 0.1, hy + 3), (x + w0 * 0.3, hy + 3), (x + w0 / 2, gy)], True), C["sky"],
            0.6, edge=0)
    pg.ink(P(tower[:2]) + P(tower[2:]), 0.7 if detail else 0.5)
    # nacelle behind the hub (the rotor faces the wind, coming from the left)
    nl = hub_h * 0.11
    nac = [(hx - nl * 0.15, hy - nl * 0.22), (hx + nl, hy - nl * 0.25), (hx + nl * 1.05, hy + nl * 0.2),
           (hx - nl * 0.1, hy + nl * 0.22)]
    pg.wash(P(nac, True), C["greyl"], 0.85)
    pg.ink(P(nac, True), 0.6 if detail else 0.45)
    for k in range(3):
        a = phase + k * 2 * math.pi / 3
        bl = _blade(hx - nl * 0.2, hy, s_blade, a)
        pg.wash(S(bl, True, 0.3), C["white"], 0.9, edge=0.5)
        pg.ink(S(bl, True, 0.3), 0.6 if detail else 0.4)
    pg.wash(ell(hx - nl * 0.25, hy, nl * 0.22, nl * 0.32), C["greyl"], 0.9)
    pg.ink(ell(hx - nl * 0.25, hy, nl * 0.22, nl * 0.32), 0.6 if detail else 0.4)
    return hx - nl * 0.2, hy, nac


def wind_turbines():
    pg = Page(7)
    gy = 160
    # prairie with fields
    pg.vignette(4, 132, 214, 168, C["ochrel"], 0.5, 3, seed=1)
    pg.ink(S([(4, 140), (60, 138), (120, 141), (214, 137)]), 0.5, INK2)
    pg.ink(P([(10, 152), (60, 148)]) + P([(90, 156), (170, 150)]) + P([(120, 146), (200, 143)]), 0.35, INK2, 0.6)
    # receding turbines, all turned to the same wind
    for x, h, b, ph in ((150, 54, 24, 0.5), (178, 40, 18, 1.2), (200, 30, 13, 0.2)):
        _turbine(pg, x, 140 - (150 - x) * 0.02, h, b, ph, detail=False, w0=3.2 * h / 54)
    hx, hy, nac = _turbine(pg, 66, gy, 98, 48, -math.pi / 2 + 0.12)
    # wind from the left; rotation (clockwise seen from upwind)
    for y in (40, 54, 68):
        pg.arrow([(6, y), (16, y - 1), (26, y)], BLUE_ARROW, 0.9, 2.8)
    rot = ell_pts(hx, hy, 15, 15, 16, math.radians(-150), math.radians(-60))
    pg.arrow(rot, RED_PENCIL, 0.9, 2.8)
    # a person and a pickup truck at the foot of the tower, for scale
    pg.person(76, gy, 3.6)
    pg.scalebar(14, 172, 40, 4)
    # the nacelle opened up: hub, main shaft, gearbox, generator
    with pg.inset_circle(268, 44, 32):
        pg.wash(circ(268, 44, 32), C["skyp"], 0.4, edge=0)
        shell = [(246, 34), (296, 32), (300, 56), (246, 58)]
        pg.ink(P(shell, True), 0.7, INK2)
        pg.wash(ell(242, 46, 6, 10), C["greyl"], 0.9)
        pg.ink(ell(242, 46, 6, 10), 0.7)
        pg.ink(P([(236, 38), (226, 20)]) + P([(236, 54), (226, 72)]), 0.7)               # blade roots
        pg.wash(rect(248, 44, 12, 4), C["grey"], 0.9)
        pg.ink(rect(248, 44, 12, 4), 0.6)                                                # main shaft
        pg.wash(rect(260, 38, 13, 16), C["sienna"], 0.7)
        pg.ink(rect(260, 38, 13, 16), 0.6)                                               # gearbox
        pg.ink(P([(273, 46), (278, 46)]), 0.8)
        pg.wash(rect(278, 39, 15, 14), C["sap"], 0.7)
        pg.ink(rect(278, 39, 15, 14) + P([(282, 39), (282, 53)]) + P([(286, 39), (286, 53)]), 0.6)  # generator
        pg.ink(P([(268, 58), (268, 76)]) + P([(262, 58), (274, 58)]), 0.6)               # yaw bearing and tower
    pg.leader((hx + 8, hy - 2), (236, 40), bend=-0.08)
    # turbines have grown: early, middle and recent machines side by side
    with pg.inset_box(234, 96, 78, 70):
        gb = 160
        pg.ink(P([(236, gb), (310, gb)]), 0.5, INK2)
        for x, h, b in ((248, 16, 7), (268, 28, 12), (293, 44, 18)):
            pg.ink(P([(x, gb), (x, gb - h)]), 0.7)
            for k in range(3):
                a = -math.pi / 2 + k * 2 * math.pi / 3 + 0.2
                pg.ink(P([(x, gb - h), (x + b * math.cos(a), gb - h + b * math.sin(a))]), 0.6)
            pg.dot(x, gb - h, 0.9)
        pg.arrow([(242, 108), (262, 104), (282, 100)], RED_PENCIL, 0.8, 2.6)
    return pg.svg()


# ------------------------------------------------------------------ nuclear
def _cooling_tower(pg: Page, x, gy, h, w):
    """A hyperboloid natural-draught cooling tower: half-width a*sqrt(1 + ((z - z0)/c)^2), with the
    throat at 0.78 h, so it narrows in a curve, then flares slightly at the top."""
    a, z0, c = 0.237 * w, 0.78 * h, 0.42 * h
    l, r = [], []
    for i in range(15):
        z = h * i / 14
        ww = a * math.sqrt(1 + ((z - z0) / c) ** 2)
        l.append((x - ww, gy - z))
        r.append((x + ww, gy - z))
    pg.wash(P(l + r[::-1], True), C["greyl"], 0.75, edge=0)
    pg.wash(P([(x + w * 0.12, gy)] + r + [(r[-1][0] - w * 0.12, gy - h)], True), C["slate"], 0.45, edge=0)
    pg.ink(S(l) + S(r), 0.75)
    pg.ink(ell(x, gy - h, r[-1][0] - x, 2.2), 0.6)
    pg.ink("".join(P([(x - w * 0.45 + k * w * 0.15, gy), (x - w * 0.42 + k * w * 0.15, gy - 3)]) for k in range(7)),
           0.4)
    return gy - h, r[-1][0] - x


def nuclear():
    pg = Page(8)
    gy = 136
    pg.ground(4, 214, gy + 4, C["sapl"], 6, 0.6)
    # river in front (the cooling water source)
    pg.wash(S([(4, 150), (60, 146), (120, 150), (212, 145), (212, 158), (120, 162), (60, 158), (4, 162)], True),
            C["cerul"], 0.55, edge=0)
    pg.ink(S([(4, 150), (60, 146), (120, 150), (212, 145)]), 0.5, INK2)
    pg.ink("".join(f"M{x} {154 + (x % 5) * .4:.1f}h5" for x in range(14, 200, 17)), 0.35, INK2)
    # two cooling towers with water-vapour plumes
    for x, h, w in ((46, 74, 40), (98, 66, 36)):
        top, rw = _cooling_tower(pg, x, gy, h, w)
        plume = [(x - rw, top), (x - rw - 6, top - 10), (x - rw + 2, top - 22), (x + 8, top - 30), (x + 26, top - 32),
                 (x + 40, top - 24), (x + 36, top - 12), (x + rw, top)]
        pg.wash(S(plume, True), C["white"], 0.9, edge=0)
        pg.ink(scallop(plume[:-1], 0.32), 0.6, INK2)
    # reactor containment (cylinder and dome) and turbine hall
    cx = 150
    pg.wash(P([(cx - 14, gy), (cx - 14, gy - 28), (cx + 14, gy - 28), (cx + 14, gy)], True), C["greyl"], 0.8)
    pg.wash(f"M{cx - 14} {gy - 28}a14 12 0 0 1 28 0Z", C["greyl"], 0.8)
    pg.wash(P([(cx + 3, gy), (cx + 3, gy - 28), (cx + 14, gy - 28), (cx + 14, gy)], True), C["slate"], 0.35, edge=0)
    pg.ink(P([(cx - 14, gy), (cx - 14, gy - 28)]) + P([(cx + 14, gy - 28), (cx + 14, gy)]) +
           f"M{cx - 14} {gy - 28}a14 12 0 0 1 28 0", 0.75)
    hall = [(cx + 14, gy), (cx + 14, gy - 18), (cx + 56, gy - 18), (cx + 56, gy)]
    pg.wash(P(hall, True), C["ochrel"], 0.7)
    pg.ink(P(hall, False), 0.7)
    pg.ink("".join(P([(cx + 18 + k * 7, gy - 14), (cx + 18 + k * 7, gy - 6)]) for k in range(6)), 0.35, INK2)
    # a transmission pylon carrying the power away
    px = 196
    pg.ink(P([(px - 5, gy), (px, gy - 40), (px + 5, gy)]) + P([(px - 8, gy - 30), (px + 8, gy - 30)]) +
           P([(px - 6, gy - 22), (px + 6, gy - 22)]) + P([(px - 3, gy - 12), (px + 3, gy - 20)]) +
           P([(px + 3, gy - 12), (px - 3, gy - 20)]), 0.55)
    pg.ink(f"M{cx + 50} {gy - 18}Q{(cx + 50 + px - 8) / 2} {gy - 20} {px - 8} {gy - 30}"
           f"M{px + 8} {gy - 30}Q{px + 12} {gy - 26} 214 {gy - 26}", 0.35, INK2)
    # fuel assembly cross-section: a square lattice of fuel rods with control-rod guide tubes
    with pg.inset_circle(268, 54, 34):
        pg.wash(circ(268, 54, 34), C["skyp"], 0.35, edge=0)
        n_, sp = 8, 6.4
        x0, y0 = 268 - sp * (n_ - 1) / 2, 54 - sp * (n_ - 1) / 2
        guides = {(2, 2), (5, 2), (2, 5), (5, 5), (3, 3), (4, 4), (3, 4), (4, 3)}
        for i in range(n_):
            for j in range(n_):
                x, y = x0 + i * sp, y0 + j * sp
                if (i, j) in guides:
                    pg.ink(circ(x, y, 2.4), 0.5)
                else:
                    pg.wash(circ(x, y, 2.3), C["orange"], 0.75, edge=0.5)
        pg.ink(rect(x0 - 4, y0 - 4, sp * (n_ - 1) + 8, sp * (n_ - 1) + 8), 0.6)
    pg.leader((cx, gy - 32), (240, 70), bend=-0.1)
    # a single fuel pellet with a scale bar (about a centimetre)
    pg.wash(P([(250, 120), (262, 120), (262, 140), (250, 140)], True), C["slate"], 0.7)
    pg.wash(ell(256, 120, 6, 2), C["greyl"], 0.9, edge=0)
    pg.ink(ell(256, 120, 6, 2) + P([(250, 120), (250, 140)]) + P([(262, 120), (262, 140)]) +
           f"M250 140a6 2 0 0 0 12 0", 0.65)
    pg.scalebar(272, 140, 20, 2, 2.0, vertical=True)
    return pg.svg()


# ------------------------------------------------------------------ solar
def solar():
    pg = Page(9)
    # sun and its rays, low in the sky to the south (left)
    pg.wash(circ(30, 30, 12), C["lemon"], 0.85)
    pg.ink(circ(30, 30, 12), 0.6, C["bsienna"])
    for a in range(0, 360, 30):
        r = math.radians(a)
        pg.ink(P([(30 + 15 * math.cos(r), 30 + 15 * math.sin(r)), (30 + 19 * math.cos(r), 30 + 19 * math.sin(r))]), 0.6,
               C["ochre"])
    for k in range(3):
        pg.arrow([(46 + k * 10, 40 + k * 4), (80 + k * 22, 78 + k * 12)], C["ochre"], 0.9, 3, smooth=False)
    # rolling ground
    pg.vignette(4, 92, 214, 172, C["sapl"], 0.45, 4, seed=1)
    # rows of panels tilted toward the sun; each row a long parallelogram with cells
    for r_ in range(4):
        y = 104 + r_ * 16
        x0, x1 = 30 + r_ * 6, 206 - r_ * 2
        top = [(x0 + 4, y - 7), (x1 + 4, y - 7)]
        bot = [(x1, y), (x0, y)]
        panel = top + bot
        pg.wash(P(panel, True), C["ultra"], 0.8)
        pg.wash(P([(x0 + 4, y - 7), (x1 + 4, y - 7), (x1 + 3, y - 5.6), (x0 + 3, y - 5.6)], True), C["sky"], 0.7, edge=0)
        pg.ink(P(panel, True), 0.6)
        cells = "".join(P([(x0 + (x1 - x0) * t + 4 * 1, y - 7), (x0 + (x1 - x0) * t, y)]) for t in
                        [i / 16 for i in range(1, 16)])
        pg.ink(cells + P([(x0 + 2, y - 3.5), (x1 + 2, y - 3.5)]), 0.3, C["night"], 0.8)
        # legs: short front leg, longer rear leg (so the panels face the sun)
        for t in (0.1, 0.5, 0.9):
            lx = lerp(x0, x1, t)
            pg.ink(P([(lx, y), (lx, y + 3)]) + P([(lx + 4, y - 7), (lx + 4, y + 3)]), 0.45)
    # the satellite view: the field as a machine-drawn outline around the panel rows
    with pg.inset_box(232, 18, 80, 64):
        pg.wash(rect(232, 18, 80, 64), C["sapl"], 0.5, edge=0)
        out = [(242, 30), (290, 26), (302, 44), (296, 72), (250, 74), (240, 56)]
        for k in range(10):
            y = 32 + k * 4
            xs = [p for p in out]
            pg.ink(P([(244 + (2 if k < 3 else 0), y), (296 - (4 if k > 7 else 0), y)]), 1.4, C["ultra"], 0.85)
        pg.ink(P(out, True), 0.8, RED_PENCIL, 0.95, "2.4 1.6")
        for p in out:
            pg.dot(p[0], p[1], 0.9, RED_PENCIL)
    pg.leader((150, 100), (232, 60), bend=-0.08)
    # a single cell, close up: silicon with thin silver fingers and two busbars
    with pg.inset_circle(272, 132, 30):
        pg.wash(rect(252, 112, 40, 40), C["night"], 0.75)
        pg.ink("".join(P([(252, y), (292, y)]) for y in range(115, 152, 3)), 0.25, C["white"], 0.9)
        pg.ink(P([(265, 112), (265, 152)]) + P([(279, 112), (279, 152)]), 1.0, C["white"], 0.95)
        pg.ink(rect(252, 112, 40, 40), 0.6)
    return pg.svg()


# ------------------------------------------------------------------ mines
def mines():
    pg = Page(10)
    pg.vignette(4, 40, 214, 170, C["sand"], 0.4, 5, seed=1)
    # open pit: concentric benches stepping down to a small pit lake
    cx, cy = 92, 104
    rings = [(76, 40), (64, 33), (52, 26), (40, 19), (28, 13), (16, 7.5)]
    for i, (rx, ry) in enumerate(rings):
        pg.wash(ell(cx + i * 1.2, cy + i * 2.0, rx, ry), mix(C["sienna"], C["umber"], i / 6), 0.35, edge=0)
    for i, (rx, ry) in enumerate(rings):
        x, y = cx + i * 1.2, cy + i * 2.0
        # the far wall (upper arc) is the bench face we see; draw full ellipse lightly, upper arc firmly
        pg.ink(S(ell_pts(x, y, rx, ry, 30, math.pi, 2 * math.pi)), 0.7)
        pg.ink(S(ell_pts(x, y, rx, ry, 30, 0, math.pi)), 0.4, INK2)
        # bench face hatching on the far side
        if i < len(rings) - 1:
            nx, ny = cx + (i + 1) * 1.2, cy + (i + 1) * 2.0
            rx2, ry2 = rings[i + 1]
            segs = ""
            for a in [math.pi + k * math.pi / 14 for k in range(1, 14)]:
                segs += P([(x + rx * math.cos(a), y + ry * math.sin(a)),
                           (nx + rx2 * math.cos(a) * 1.02, ny + ry2 * math.sin(a) * 1.02 - 1.5)])
            pg.ink(segs, 0.3, INK2, 0.7)
    pg.wash(ell(cx + 7, cy + 12, 12, 5), C["teal"], 0.75)
    pg.ink(ell(cx + 7, cy + 12, 12, 5), 0.5)
    # the haul road spiralling down
    road = [(cx + 74, cy - 6), (cx + 50, cy - 26), (cx, cy - 33), (cx - 46, cy - 18), (cx - 46, cy + 6), (cx - 8, cy + 26),
            (cx + 32, cy + 18), (cx + 34, cy + 2), (cx + 12, cy - 4)]
    pg.ink(S(road), 1.2, C["umberl"], 0.9)
    pg.ink(S(road), 0.35, INK, 0.8, "1.4 1.6")
    # waste dump (flat-topped) and tailings pond behind its dam
    dump = [(168, 74), (176, 58), (204, 56), (212, 74)]
    pg.wash(P(dump, True), C["umberl"], 0.75)
    pg.ink(P(dump), 0.7)
    pg.ink("".join(P([(172 + k * 4, 72), (178 + k * 4, 59)]) for k in range(7)), 0.3, INK2)
    pond = [(150, 140), (178, 132), (206, 138), (206, 156), (160, 160)]
    pg.wash(S(pond, True, 0.3), C["sapl"], 0.85)
    pg.wash(S([(158, 146), (184, 140), (202, 146), (196, 154), (164, 156)], True), C["teal"], 0.45, edge=0)
    pg.ink(S(pond, True, 0.3), 0.6)
    pg.ink(S([(150, 140), (160, 160), (206, 156)]), 1.4, C["umberl"], 0.9)      # the dam embankment
    # the whole site, traced as one outline (as the dataset does)
    outline = [(10, 104), (18, 66), (60, 56), (120, 54), (160, 50), (218, 48), (218, 166), (146, 166), (120, 148),
               (40, 146)]
    pg.ink(S(outline, True, 0.35), 0.8, RED_PENCIL, 0.9, "3 2")
    # a haul truck, with a person beside a tyre
    with pg.inset_circle(270, 70, 38):
        pg.wash(circ(270, 70, 38), C["skyp"], 0.4, edge=0)
        gy = 94
        pg.ink(P([(232, gy), (308, gy)]), 0.5, INK2)
        body = [(244, 62), (300, 58), (302, 64), (294, 78), (246, 78)]
        pg.wash(P(body, True), C["ochre"], 0.85)
        pg.ink(P(body, True), 0.7)
        cab = [(236, 64), (248, 64), (248, 76), (236, 76)]
        pg.wash(P(cab, True), C["ochre"], 0.85)
        pg.ink(P(cab, True) + rect(238, 66, 8, 5), 0.6)
        pg.ink(P([(234, 78), (300, 78)]), 0.8)
        for wx in (250, 288):
            pg.wash(circ(wx, gy - 8, 8), C["payne"], 0.85)
            pg.ink(circ(wx, gy - 8, 8) + circ(wx, gy - 8, 3), 0.6)
        pg.person(270, gy, 6.5)
    pg.leader((cx + 32, cy + 18), (236, 86), bend=0.1)
    return pg.svg()
