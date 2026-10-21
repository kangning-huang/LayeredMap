"""Transport: airports, ports, railways."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, Equi, P, S, circ, ell, ell_pts, rect, scallop,
                  lerp, lerp2, mix)


# ------------------------------------------------------------------ airports
def _light_plane(pg: Page, x, gy, s=1.0):
    """A high-wing four-seat light aircraft in side view, facing left, wheels on the ground at gy."""
    def T(pts):
        return [(x + px * s, gy + py * s) for px, py in pts]
    fus = T([(-30, -13), (-26, -17), (-14, -19), (-4, -20), (8, -17), (34, -15), (40, -16), (40, -13), (10, -9),
             (-14, -8), (-28, -10)])
    pg.wash(S(fus, True, 0.3), C["white"], 0.9, edge=0.5)
    pg.wash(S(T([(-28, -11), (-14, -9), (10, -10), (38, -13.5), (40, -13), (10, -9), (-14, -8), (-28, -10)]), True),
            C["red"], 0.8, edge=0)
    pg.ink(S(fus, True, 0.3), 0.7)
    fin = T([(28, -15.5), (34, -30), (40, -30), (41, -16)])
    pg.wash(P(fin, True), C["white"], 0.9, edge=0.5)
    pg.ink(P(fin), 0.7)
    pg.ink(P(T([(30, -14.5), (46, -15)])), 0.9)                               # horizontal tail, edge-on
    win = T([(-16, -18.5), (-6, -19.5), (2, -17.5), (-2, -13), (-16, -13)])
    pg.wash(P(win, True), C["sky"], 0.85, edge=0)
    pg.ink(P(win, True) + P(T([(-9, -19.2), (-9, -13)])), 0.5)
    # high wing on top (seen edge-on) and its strut down to the fuselage
    pg.ink(P(T([(-20, -21), (12, -21)])), 1.6)
    pg.ink(P(T([(-2, -21), (-12, -9)])), 0.5)
    # propeller, spinner, landing gear (tricycle)
    pg.ink(P(T([(-31, -21), (-31, -3)])) + P(T([(-30, -13), (-33, -12)])), 0.7)
    pg.ink(P(T([(-24, -9), (-24, -2.5)])) + P(T([(-6, -8), (-2, -2.5)])), 0.6)
    for wx in (-24, -1.5):
        px, py = T([(wx, -2.5)])[0]
        pg.wash(circ(px, py, 2.4 * s), C["payne"], 0.85)
        pg.ink(circ(px, py, 2.4 * s), 0.5)


def airports():
    pg = Page(14)
    hz = 72
    pg.vignette(4, 20, 212, hz, C["skyp"], 0.4, 4, seed=1)
    pg.vignette(4, hz, 212, 170, C["sapl"], 0.5, 4, seed=2)
    pg.ink(S([(4, hz), (60, hz - 1), (120, hz + 0.5), (212, hz - 0.5)]), 0.5, INK2)
    # runway in perspective, with threshold stripes and a dashed centreline (no numbers)
    vp = (150, hz - 2)
    near_l, near_r = (82, 172), (196, 172)
    far_l, far_r = lerp2(near_l, vp, 0.86), lerp2(near_r, vp, 0.86)
    rw = [near_l, near_r, far_r, far_l]
    pg.wash(P(rw, True), C["greyl"], 0.8)
    pg.ink(P(rw, True), 0.6)
    for k in range(6):
        a = lerp2(near_l, near_r, 0.1 + k * 0.15)
        b = lerp2(a, vp, 0.08)
        pg.wash(P([a, (a[0] + 6, a[1]), (b[0] + 4.6, b[1]), b], True), C["white"], 0.95, edge=0)
    pg.ink(P([lerp2((139, 172), vp, 0.12), lerp2((139, 172), vp, 0.84)]), 0.7, C["white"], 0.95, "5 4")
    # a hangar (quonset) and the windsock
    pg.wash(f"M14 {hz + 8}a20 16 0 0 1 40 0Z", C["greyl"], 0.85)
    pg.ink(f"M14 {hz + 8}a20 16 0 0 1 40 0Z" + "".join(f"M{x} {hz + 8}v-{14 - abs(x - 34) * 0.4:.1f}" for x in (24, 34, 44)),
           0.55)
    pg.ink(P([(196, hz + 24), (196, hz - 6)]), 0.7)
    sock = [(196, hz - 6), (214, hz - 4), (214, hz), (196, hz - 1)]
    pg.wash(P(sock, True), C["orange"], 0.85)
    pg.ink(P(sock, True) + P([(202, hz - 5.3), (202, hz - 1.4)]) + P([(208, hz - 4.6), (208, hz - 0.6)]), 0.5)
    pg.arrow([(162, hz - 14), (176, hz - 14), (190, hz - 12)], BLUE_ARROW, 0.8, 2.6)
    _light_plane(pg, 46, 140, 1.15)
    pg.ground(6, 100, 141, C["hooker"], 3, 0.35)
    # insets: the more unusual places to land, a heliport and a balloonport
    with pg.inset_circle(268, 50, 32):
        pg.wash(circ(268, 50, 32), C["skyp"], 0.4, edge=0)
        # helicopter, side view
        body = [(252, 54), (256, 46), (270, 44), (280, 48), (282, 56), (268, 60), (256, 59)]
        pg.wash(S(body, True), C["ultra"], 0.75)
        pg.wash(S([(256, 47), (266, 45), (266, 54), (254, 54)], True), C["sky"], 0.85, edge=0)
        pg.ink(S(body, True), 0.65)
        pg.ink(P([(280, 50), (292, 48)]) + P([(292, 48), (294, 43)]) + P([(266, 44), (266, 40)]), 0.65)
        pg.ink(P([(242, 40), (290, 39)]), 0.9)                                         # main rotor
        pg.ink(circ(293, 46, 3.4), 0.4)                                                # tail rotor
        pg.ink(P([(256, 64), (280, 64)]) + P([(260, 60), (259, 64)]) + P([(276, 59), (277, 64)]), 0.6)  # skids
        pg.ink(P([(238, 70), (298, 70)]), 0.5, INK2)
        pg.ink(ell(268, 72, 20, 3), 0.5, INK2)                                        # pad circle
    with pg.inset_circle(268, 130, 32):
        pg.wash(circ(268, 130, 32), C["skyp"], 0.4, edge=0)
        env = [(268, 102), (282, 108), (286, 122), (278, 136), (272, 142), (264, 142), (258, 136), (250, 122),
               (254, 108)]
        for k, c in enumerate((C["red"], C["ochre"], C["red"], C["ochre"])):
            x0 = 252 + k * 8.5
            pg.wash(P([(x0, 112), (x0 + 8.5, 112), (x0 + 6, 140), (x0 + 3, 140)], True), c, 0.7, edge=0)
        pg.ink(S(env, True), 0.7)
        pg.ink(S([(268, 102), (264, 120), (266, 142)]) + S([(268, 102), (274, 120), (270, 142)]), 0.4)
        pg.ink(P([(264, 142), (264, 148)]) + P([(272, 142), (272, 148)]), 0.5)
        pg.wash(rect(262, 148, 12, 7), C["umberl"], 0.85)
        pg.ink(rect(262, 148, 12, 7), 0.6)
    pg.scalebar(14, 172, 40, 4)
    return pg.svg()


# ------------------------------------------------------------------ ports
def ports():
    """A berth seen along the quay: the ship end-on in the water, a ship-to-shore crane on the quay
    with its boom out over the ship, the channel depth and the keel clearance below."""
    pg = Page(15)
    wl, qy, bed = 122, 116, 166
    qx = 150                                   # quay wall
    # water, seabed, quay
    pg.wash(P([(4, wl), (qx, wl), (qx, bed), (4, bed)], True), C["cerul"], 0.5, edge=0)
    pg.wash(S([(4, bed - 2), (40, bed), (80, bed - 1), (120, bed + 1), (qx, bed), (qx, 172), (4, 172)], True), C["sand"],
            0.8, edge=0)
    pg.ink(P([(4, wl), (qx, wl)]), 0.6)
    pg.ink(S([(4, bed - 2), (40, bed), (80, bed - 1), (120, bed + 1), (qx, bed)]), 0.55, INK2)
    pg.ink("".join(f"M{x} {wl + 6 + (x % 3) * 5}h5" for x in range(10, 60, 13)), 0.35, INK2)
    pg.vignette(qx, qy, 214, 172, C["greyl"], 0.7, 2, seed=3)
    pg.ink(P([(qx, bed + 2), (qx, qy), (214, qy)]), 0.8)
    pg.hatch([(qx, qy), (214, qy), (214, 170), (qx, 170)], -45, 3.0, 0.3, INK2, 0.6)
    # the ship, end-on: hull section, containers on deck
    hull = [(66, 90), (140, 90), (140, 140), (134, 148), (72, 148), (66, 140)]
    pg.wash(P(hull, True), C["crimson"], 0.6)
    pg.wash(P([(66, 90), (140, 90), (140, wl), (66, wl)], True), C["payne"], 0.25, edge=0)
    pg.ink(P(hull, True), 0.75)
    pg.wash(P([(66, wl), (140, wl), (140, 140), (134, 148), (72, 148), (66, 140)], True), C["cerul"], 0.45, edge=0)
    rng = random.Random(15)
    cols = [C["red"], C["ultra"], C["ochre"], C["sap"], C["bsienna"], C["cerul"]]
    stack = ""
    for i in range(6):
        for j in range(4 if i in (0, 5) else 5):
            x, y = 68 + i * 12, 90 - (j + 1) * 6.4
            pg.wash(rect(x, y, 11.4, 6), rng.choice(cols), 0.7, edge=0)
            stack += rect(x, y, 11.4, 6)
    pg.ink(stack, 0.35)
    # ship-to-shore crane: waterside and landside legs on the quay rails, portal, boom and A-frame
    l1, l2, top = 162, 200, 44
    legs = P([(l1, qy), (l1, top)]) + P([(l2, qy), (l2, top)]) + P([(l1, top + 26), (l2, top + 26)]) + \
        P([(l1, top + 26), (l2, top + 52)]) + P([(l2, top + 26), (l1, top + 52)]) + P([(l1, top + 52), (l2, top + 52)])
    pg.ink(legs, 0.85)
    pg.wash(rect(l1 - 1.5, top, 3, qy - top) + rect(l2 - 1.5, top, 3, qy - top), C["red"], 0.55, edge=0)
    boom_l, boom_r, by = 30, 214, top - 4
    pg.ink(P([(boom_l, by), (boom_r, by)]) + P([(boom_l, by + 4), (boom_r, by + 4)]) +
           "".join(P([(x, by), (x + 6, by + 4)]) for x in range(34, 210, 12)), 0.7)
    apex = (190, 10)
    pg.ink(P([(l1, top), apex, (l2, top)]) + P([apex, (boom_l + 8, by)]) + P([apex, (boom_r, by)]), 0.55)
    pg.ink(P([(l1 - 4, qy), (l1 + 4, qy)]) + P([(l2 - 4, qy), (l2 + 4, qy)]), 1.4)          # rail bogies
    pg.wash(rect(200, by + 4, 12, 9), C["red"], 0.75, edge=0)
    pg.ink(rect(200, by + 4, 12, 9), 0.6)                                                # machinery house
    # trolley over the ship, hoist ropes down to the spreader and a container
    tx = 104
    pg.ink(rect(tx - 5, by + 4, 10, 3), 0.6)
    pg.ink(P([(tx - 3, by + 7), (tx - 4, 52)]) + P([(tx + 3, by + 7), (tx + 4, 52)]), 0.45)
    pg.ink(P([(tx - 7, 52), (tx + 7, 52)]), 1.0)
    pg.wash(rect(tx - 6, 53, 12, 6), C["ochre"], 0.9)
    pg.ink(rect(tx - 6, 53, 12, 6), 0.5)
    pg.arrow([(tx + 12, 66), (tx + 12, 54)], RED_PENCIL, 0.8, 2.4, both=True, smooth=False)
    pg.person(208, qy, 6)
    # channel depth (water surface to bed) and the clearance under the keel
    pg.arrow([(30, wl + 1), (30, bed - 1)], RED_PENCIL, 0.9, 2.8, both=True, smooth=False)
    pg.arrow([(103, 149), (103, bed - 1)], BLUE_ARROW, 0.8, 2.4, both=True, smooth=False)
    # ships have outgrown many harbours: three hulls side by side, small to large
    with pg.inset_box(230, 96, 82, 72):
        pg.wash(rect(230, 96, 82, 72), C["sky"], 0.35, edge=0)
        for k, (L, hh) in enumerate(((30, 5), (50, 6.5), (74, 8))):
            y = 112 + k * 20
            x0 = 234
            sh = [(x0, y - hh), (x0 + L, y - hh), (x0 + L + 4, y - hh - 2), (x0 + L, y), (x0 + 3, y)]
            pg.wash(P(sh, True), C["crimson"], 0.6)
            pg.ink(P(sh, True), 0.55)
            pg.wash(rect(x0 + 2, y - hh - 4, 4, 4), C["white"], 0.9, edge=0.5)
            for c in range(int(L / 7)):
                pg.wash(rect(x0 + 8 + c * 6.5, y - hh - 3, 6, 3), cols[(c + k) % 6], 0.7, edge=0)
        pg.arrow([(300, 104), (300, 156)], RED_PENCIL, 0.8, 2.6, smooth=False)
    return pg.svg()


# ------------------------------------------------------------------ railways (Japan)
def railways():
    pg = Page(16)
    # a symmetric snow-capped volcano behind
    fuji = [(20, 96), (70, 60), (92, 34), (104, 30), (116, 34), (138, 60), (190, 96)]
    pg.wash(S(fuji, True, 0.25), C["sky"], 0.55)
    snow = [(85, 42), (92, 34), (104, 30), (116, 34), (123, 42), (117, 46), (111, 41), (104, 47), (97, 41), (90, 47)]
    pg.wash(S(snow, True, 0.2), C["white"], 0.95, edge=0)
    pg.ink(S(fuji, False, 0.25), 0.6, INK2)
    pg.ink(P(snow[4:] + [snow[0]]), 0.35, INK2)
    # rice paddies in front, flooded and reflecting
    pg.vignette(4, 108, 212, 170, C["sapl"], 0.55, 3, seed=1)
    for y in (126, 140, 156):
        pg.ink(S([(6, y), (70, y - 2), (140, y + 1), (210, y - 1)]), 0.4, INK2)
    pg.wash(S([(8, 132), (70, 130), (140, 133), (208, 131), (208, 137), (140, 139), (70, 136), (8, 138)], True), C["sky"],
            0.5, edge=0)
    # a viaduct: deck on piers; every pier stands on the ground and reaches the deck
    deck_y = 98
    pg.wash(rect(4, deck_y, 210, 5), C["greyl"], 0.9)
    pg.ink(rect(4, deck_y, 210, 5), 0.7)
    piers = ""
    for x in range(14, 212, 22):
        pg.wash(rect(x - 2, deck_y + 5, 4, 118 - deck_y - 5), C["greyl"], 0.8, edge=0)
        piers += P([(x - 2, deck_y + 5), (x - 2, 118)]) + P([(x + 2, deck_y + 5), (x + 2, 118)])
        piers += P([(x - 4, 118), (x + 4, 118)])
    pg.ink(piers, 0.6)
    # high-speed train on the deck, long nose to the right
    car = [(16, deck_y), (16, 84), (20, 80), (166, 80), (186, 84), (200, 92), (204, deck_y)]
    pg.paper(P(car, True))
    pg.wash(P(car, True), C["white"], 0.95, edge=0.5)
    pg.wash(P([(16, 92), (200, 92), (204, deck_y), (16, deck_y)], True), C["ultra"], 0.75, edge=0)
    pg.ink(S(car, False, 0.15) + P([(16, deck_y), (204, deck_y)]), 0.75)
    pg.ink("".join(P([(x, 80.5), (x, deck_y)]) for x in (52, 88, 124, 160)), 0.4)    # car joints
    pg.ink(P([(22, 86), (164, 86)]), 1.4, INK2, 0.85, "2 1.6")                        # window band
    pg.wash(P([(178, 84), (188, 85.5), (186, 88), (176, 87)], True), C["payne"], 0.85, edge=0)
    pg.ink(P([(60, 80), (64, 76), (72, 76)]), 0.5)                                    # pantograph
    pg.arrow([(150, 70), (176, 70), (200, 72)], RED_PENCIL, 0.9, 3)
    # inset: a line diagram with stations, two lines meeting at an interchange
    with pg.inset_box(230, 18, 82, 70):
        l1 = [(236, 78), (252, 66), (268, 54), (286, 46), (306, 30)]
        l2 = [(238, 34), (256, 44), (268, 54), (282, 66), (304, 74)]
        pg.ink(S(l1), 2.0, C["ultra"], 0.85)
        pg.ink(S(l2), 2.0, C["red"], 0.85)
        for p in l1[:2] + l1[3:] + l2[:2] + l2[3:]:
            pg.raw(f'<circle cx="{p[0]}" cy="{p[1]}" r="2" fill="{C["white"]}" stroke="{INK}" stroke-width=".6"/>')
        pg.raw(f'<circle cx="268" cy="54" r="3.2" fill="{C["white"]}" stroke="{INK}" stroke-width=".8"/>')
    # and the islands, with the main trunk line
    m = Equi(128.5, 146, 30.5, 42, 236, 100, 70)
    with pg.inset_box(236, 100, 70, m.h):
        pg.wash(rect(236, 100, 70, m.h), C["sky"], 0.45, edge=0)
        pg.land(m.land(tol=0.9, only={"JPN"}), C["sapl"], 0.9)
        trunk = [m(130.4, 33.6), m(132.5, 34.4), m(135.5, 34.7), m(136.9, 35.2), m(139.7, 35.7), m(140.9, 38.3),
                 m(141.2, 40.8)]
        pg.ink(S(trunk), 0.9, RED_PENCIL, 0.95)
        for p in trunk:
            pg.dot(p[0], p[1], 0.9)
    pg.leader((120, 88), (230, 60), bend=-0.08)
    return pg.svg()
