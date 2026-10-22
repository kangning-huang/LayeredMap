"""From the sky: meteorites, the 2024 eclipse, Earth at night."""
import math
import random

from .kit import (C, INK, INK2, RED_PENCIL, BLUE_ARROW, PAPER, Page, Equi, Ortho, P, S, circ, ell, ell_pts, rect,
                  scallop, lerp, lerp2, mix, cities, hatch_lines, segs_d)


# ------------------------------------------------------------------ meteorites
def meteorites():
    pg = Page(27)
    # a fireball streak high in the sky, breaking up
    pg.wash(P([(10, 14), (70, 44), (72, 41)], True), C["ochre"], 0.6, edge=0)
    pg.ink(P([(14, 16), (70, 42)]), 0.5, C["bsienna"], 0.8)
    pg.wash(circ(72, 43, 3), C["lemon"], 0.95)
    pg.ink(circ(72, 43, 3), 0.5, C["bsienna"])
    for dx, dy in ((7, 6), (9, 1), (4, 9)):
        pg.dot(72 + dx, 43 + dy, 0.8, C["bsienna"])
    # an iron meteorite: rounded mass, fusion crust, thumb-print hollows (regmaglypts)
    body = [(38, 132), (34, 112), (44, 90), (66, 76), (98, 72), (126, 80), (146, 96), (152, 118), (142, 136), (112, 146),
            (70, 146)]
    pg.ground(30, 170, 148, C["greyl"], 4, 0.6)
    pg.wash(S(body, True, 0.4), C["umber"], 0.8)
    pg.wash(S([(100, 76), (126, 80), (146, 96), (152, 118), (142, 136), (112, 146), (124, 120), (118, 96)], True),
            "#3b3f47", 0.45, edge=0)
    pg.wash(S([(50, 96), (70, 84), (92, 82), (80, 96), (60, 104)], True), C["sienna"], 0.55, edge=0)     # rust bloom
    pg.ink(S(body, True, 0.4), 0.85)
    rng = random.Random(27)
    hol = ""
    for x, y, r in ((62, 104, 9), (88, 96, 8), (112, 100, 9), (74, 126, 8), (102, 124, 9), (128, 118, 6), (52, 122, 6),
                    (132, 98, 5), (90, 140, 5)):
        hol += S(ell_pts(x, y, r, r * 0.75, 10, 0.5, 3.6))
        pg.wash(S(ell_pts(x + 1, y + 1, r * 0.8, r * 0.6, 12), True), "#3b3f47", 0.3, edge=0)
    pg.ink(hol, 0.5)
    pg.raw(f'<path d="M58 88q8-6 18-6" stroke="{C["white"]}" stroke-width="1" stroke-opacity=".7" fill="none"/>')
    pg.scalebar(52, 162, 80, 4)
    # a cut, polished and etched slice: the Widmanstatten pattern of crossing nickel-iron plates
    with pg.inset_circle(256, 62, 44):
        cx, cy, r = 256, 62, 44
        pg.wash(circ(cx, cy, r), C["greyl"], 0.65, edge=0)
        poly = ell_pts(cx, cy, r, r, 24)
        d = ""
        for ang, gap in ((30, 4.6), (90, 5.2), (150, 4.8)):
            segs = hatch_lines(poly, ang, gap, None, 0)
            out = []
            for (a, b) in segs:
                # break each plate into pieces, the way the bands interrupt one another
                L = math.hypot(b[0] - a[0], b[1] - a[1])
                t = 0
                while t < L:
                    l = rng.uniform(6, 16)
                    if rng.random() < 0.7:
                        out.append((lerp2(a, b, t / L), lerp2(a, b, min(1, (t + l) / L))))
                    t += l + rng.uniform(1, 4)
            d += segs_d(out)
        pg.ink(d, 0.7, C["payne"], 0.85)
        pg.ink(d, 0.3, C["white"], 0.6)
    pg.leader((138, 92), (214, 74), bend=-0.08)
    # stony meteorites hold chondrules: small round grains in a fine matrix
    with pg.inset_box(226, 122, 86, 44):
        pg.wash(rect(226, 122, 86, 44), C["greyl"], 0.5, edge=0)
        d = ""
        for i in range(26):
            x, y, r = rng.uniform(230, 308), rng.uniform(126, 162), rng.uniform(1.4, 4.2)
            pg.wash(circ(x, y, r), rng.choice([C["umberl"], C["sand"], C["slate"]]), 0.7, edge=0)
            d += circ(x, y, r)
        pg.ink(d, 0.4)
    return pg.svg()


# ------------------------------------------------------------------ eclipse (8 April 2024)
def _phase(pg: Page, x, y, r, f):
    """The Sun at (x, y) with the Moon covering it by |f| (0 none, 1 total). The Moon crosses from
    west to east (right to left, facing south): f > 0 before totality (Moon on the right), f < 0 after."""
    if abs(f) >= 1:
        for a in range(0, 360, 30):
            ra = math.radians(a + 8)
            L = r * (1.9 if a % 90 == 0 else 1.45)
            pg.ink(S([(x + math.cos(ra) * r, y + math.sin(ra) * r),
                      (x + math.cos(ra + 0.08) * L * 0.75, y + math.sin(ra + 0.08) * L * 0.75),
                      (x + math.cos(ra) * L, y + math.sin(ra) * L)]), 0.5, C["slate"], 0.8)
        pg.wash(circ(x, y, r * 1.35), C["sky"], 0.55, edge=0)
        pg.ink_fill(circ(x, y, r), C["night"], 0.95)
        pg.ink_fill(circ(x - r * 0.75, y - r * 0.68, 0.9), C["red"])
        pg.ink_fill(circ(x + r * 0.96, y + r * 0.2, 0.7), C["red"])
        return
    pg.wash(circ(x, y, r), C["lemon"], 0.9)
    if f == 0:
        pg.ink(circ(x, y, r), 0.5, C["bsienna"])
        return
    off = 2 * r * (1 - abs(f)) * (1 if f > 0 else -1)
    mx, my = x + off, y - abs(off) * 0.12
    cid = pg.uid("k")
    pg.defs.append(f'<clipPath id="{cid}"><path d="{circ(x, y, r + 0.6)}"/></clipPath>')
    pg.raw(f'<path d="{circ(mx, my, r * 1.03)}" fill="#e8edee" clip-path="url(#{cid})"/>')
    # the lit crescent's edge in pen, the Moon's limb as a pencil ghost
    pg.ink(circ(x, y, r), 0.5, C["bsienna"])
    pg.ink(circ(mx, my, r * 1.03), 0.35, INK2, 0.7, "1.2 1.2")


def eclipse():
    pg = Page(28)
    gy = 138
    # a hill-line horizon and the Sun's path across the sky through the eclipse
    pg.vignette(4, 10, 214, gy, C["skyp"], 0.35, 4, seed=1)
    pg.vignette(4, gy - 2, 214, 170, C["sapl"], 0.5, 3, seed=2)
    pg.ink(S([(4, gy + 2), (50, gy - 6), (100, gy - 1), (150, gy - 8), (214, gy - 2)]), 0.6, INK2)
    arc = [(24, 120), (50, 82), (80, 56), (110, 44), (140, 50), (168, 70), (192, 100)]
    pg.ink(S(arc), 0.4, INK2, 0.6, "1.6 2")
    for (x, y), f in zip(arc, (0, 0.35, 0.7, 1.0, -0.7, -0.35, 0)):
        _phase(pg, x, y, 8.5, f)
    pg.arrow([(116, 30), (134, 32)], INK, 0.6, 2.2)
    # trees on the horizon
    for x in (30, 40, 172, 182, 190):
        t = [(x - 4, gy - 3), (x, gy - 14), (x + 4, gy - 3)]
        pg.wash(P(t, True), C["hooker"], 0.7, edge=0)
        pg.ink(P(t, True), 0.45)
    # the geometry: Sun, Moon, Earth; the umbra (total) inside the penumbra (partial)
    bx, by, bw, bh = 226, 14, 86, 56
    with pg.inset_box(bx, by, bw, bh):
        sx, sy = bx - 30, by + bh / 2
        pg.wash(circ(sx, sy, 40), C["lemon"], 0.85)
        pg.ink(circ(sx, sy, 40), 0.6, C["bsienna"])
        mx, my, mr = bx + 40, sy, 5
        p_top, p_bot = (bx + 2, sy - 14), (bx + 2, sy + 14)
        apex = (bx + 61, sy)                       # where the two outer tangents meet: the umbra's tip
        ex, ey, er = apex[0] + 10, sy, 10
        pg.wash(circ(ex, ey, er), C["cerul"], 0.75)
        pg.ink(circ(ex, ey, er), 0.6)
        pg.wash(P([(mx, my - mr), apex, (mx, my + mr)], True), C["night"], 0.85, edge=0)
        pg.wash(P([(mx, my - mr), (bx + 64, sy - 17), (bx + 64, sy + 17), (mx, my + mr)], True), C["slate"], 0.3, edge=0)
        pg.ink(P([p_top, apex]) + P([p_bot, apex]), 0.4, INK2, 0.9)
        pg.ink(P([p_top, (bx + 64, sy + 17)]) + P([p_bot, (bx + 64, sy - 17)]), 0.35, INK2, 0.8, "1.4 1.4")
        pg.ink_fill(circ(mx, my, mr), C["payne"])
    # the path of totality across North America, Mexico to Maine
    m = Equi(-112, -62, 20, 50, 226, 80, 86)
    with pg.inset_box(226, 80, 86, m.h):
        pg.wash(rect(226, 80, 86, m.h), C["sky"], 0.45, edge=0)
        pg.land(m.land(tol=1.0), C["sand"], 0.9)
        line = [(-106.4, 23.2), (-103.4, 25.5), (-100.5, 29.0), (-96.8, 32.8), (-92.3, 34.7), (-89.0, 37.0),
                (-86.2, 39.8), (-81.7, 41.5), (-78.9, 42.9), (-73.2, 44.5), (-67.8, 46.1)]
        pts = [m(*p) for p in line]
        pg.stroke_wash(S(pts), C["night"], 5.0, 0.55)
        pg.ink(S(pts), 0.5, INK, 0.9)
        for k in (-1, 1):
            pg.ink(S([(x + k * 1.6, y + k * 1.6) for x, y in pts]), 0.3, INK2, 0.8, "1 1.2")
    pg.leader((110, 44), (226, 100), bend=0.12)
    return pg.svg()


# ------------------------------------------------------------------ night lights
def night_lights():
    pg = Page(29)
    lon0, lat0 = 20, 28
    o = Ortho(lon0, lat0, 92, 92, 70)
    term = -12          # terminator meridian (an equinox evening): night to the east of it
    with pg.inset_circle(o.cx, o.cy, o.r, ring=0.9):
        pg.wash(circ(o.cx, o.cy, o.r), C["skyp"], 0.7, edge=0)
        pg.land(o.land(tol=1.8), C["sand"], 0.85)
        # night side: at an equinox the terminator is the great circle through the poles along
        # meridians term and term + 180; its visible half is half an ellipse, closed along the limb
        gc = []
        for i in range(120):
            t = 2 * math.pi * i / 120
            z = math.cos(t)
            xy = math.sin(t)
            lat = math.degrees(math.asin(max(-1, min(1, z))))
            lon = term if xy >= 0 else term + 180
            gc.append(o(lon, lat))
        k0 = next(i for i in range(120) if not gc[i][2])
        gc = gc[k0:] + gc[:k0]
        vis = [(x, y) for x, y, v in gc if v]
        e1, e2 = vis[0], vis[-1]
        nx, ny, _ = o(term + 90, 0)
        an = math.atan2(ny - o.cy, nx - o.cx)
        a1 = math.atan2(e2[1] - o.cy, e2[0] - o.cx)
        a2 = math.atan2(e1[1] - o.cy, e1[0] - o.cx)
        # go from e2 round to e1 the way that passes the night side's centre direction
        span = (a2 - a1) % (2 * math.pi)
        if (an - a1) % (2 * math.pi) > span:
            span -= 2 * math.pi
        limb = ell_pts(o.cx, o.cy, o.r + 2, o.r + 2, 30, a1, a1 + span)
        tpts = vis
        pg.wash(P(vis + limb, True), C["night"], 0.72, edge=0)
        rng = random.Random(29)
        glow, core = [], []
        for lon, lat, pop, _ in cities():
            if lon <= term + 2:
                continue
            x, y, v = o(lon, lat)
            if not v or pop < 1_000_000:
                continue
            r = 0.4 + math.sqrt(pop / 1e6) * 0.5
            glow.append((x, y, r * 1.8))
            core.append((x, y, min(r, 2.0) * 0.55))
        pg.ink(o.land(tol=2.4), 0.35, C["sky"], 0.45)
        pg.dots_sized(glow, C["orange"], 0.3, layer="w", step=0.4)
        pg.dots_sized(core, C["lemon"], None, layer="r", step=0.3)
        pg.ink(S(tpts), 0.4, C["sky"], 0.7, "1.6 1.6")
    # a polar-orbiting satellite, scanning a swath of the night side
    with pg.inset_circle(268, 44, 30):
        pg.wash(circ(268, 44, 30), C["night"], 0.7, edge=0)
        pg.wash(rect(262, 36, 12, 10), C["greyl"], 0.95)
        pg.ink(rect(262, 36, 12, 10), 0.6, C["white"])
        for sx in (-1, 1):
            pan = rect(268 + sx * 7 - (14 if sx < 0 else 0), 38, 14, 6)
            pg.wash(pan, C["ultra"], 0.9)
            pg.ink(pan + P([(268 + sx * 6, 41), (268 + sx * 7, 41)]), 0.5, C["white"])
        pg.ink(P([(266, 46), (252, 74)]) + P([(270, 46), (284, 74)]), 0.4, C["lemon"], 0.9, "1.4 1.4")
        pg.raw("".join(f'<circle cx="{x}" cy="{y}" r=".8" fill="{C["lemon"]}"/>' for x, y in
                       ((258, 66), (264, 70), (270, 68), (276, 66), (262, 62), (274, 71))))
    # one coast, sketched from a hill at night: dark sea, a glowing city on the shore, its lights
    # thinning inland along a road, reflections on the water
    bx, by, bw, bh = 234, 96, 78, 66
    with pg.inset_box(bx, by, bw, bh):
        hz = by + 20
        pg.wash(rect(bx, by, bw, hz - by + 1), C["night"], 0.8, edge=0)              # sky
        pg.raw("".join(f'<circle cx="{x}" cy="{y}" r=".45" fill="{C["white"]}"/>' for x, y in
                       ((240, 101), (251, 99), (262, 104), (279, 100), (296, 103), (306, 99), (288, 107))))
        pg.raw(f'<path d="M300 104a3.4 3.4 0 1 0 0 .1z" fill="{C["white"]}" fill-opacity=".9"/>')
        coast = [(bx, hz + 22), (250, hz + 16), (262, hz + 6), (276, hz + 2), (bx + bw, hz)]
        sea = [(bx, hz), (bx + bw, hz)] + coast[::-1]
        land = coast + [(bx + bw, by + bh), (bx, by + bh)]
        pg.wash(P([(bx, hz), (bx + bw, hz), (bx + bw, by + bh), (bx, by + bh)], True), C["night"], 0.55, edge=0)
        pg.wash(S(sea, True, 0.2), C["night"], 0.55, edge=0)                       # sea darker than land
        pg.wash(S(land, True, 0.2), C["umber"], 0.45, edge=0)
        pg.ink(S(coast), 0.5, C["sky"], 0.6)
        pg.ink(P([(bx, hz), (bx + bw, hz)]), 0.4, C["sky"], 0.5)
        # the city's glow on the shore, then lights along the coast road and inland
        pg.wash(ell(270, hz + 9, 16, 6), C["orange"], 0.75, edge=0)
        pg.wash(ell(270, hz + 9, 7, 3), C["lemon"], 0.9, edge=0)
        rng = random.Random(9)
        core = [(270 + rng.gauss(0, 6), hz + 9 + rng.gauss(0, 2.2)) for _ in range(40)]
        road = [lerp2((262, hz + 13), (bx + 6, by + bh - 4), t) for t in [i / 14 for i in range(15)]]
        road += [lerp2((282, hz + 10), (bx + bw - 4, by + bh - 8), t) for t in [i / 9 for i in range(10)]]
        towns = [(250, hz + 30), (300, hz + 30), (288, hz + 40)]
        pg.dots(core, 0.55, C["lemon"], layer="r")
        pg.dots(road, 0.45, C["ochre"], layer="r")
        for tx, ty in towns:
            pg.wash(circ(tx, ty, 3.4), C["orange"], 0.6, edge=0)
            pg.dots([(tx + rng.gauss(0, 1.3), ty + rng.gauss(0, 0.8)) for _ in range(6)], 0.45, C["lemon"], layer="r")
        # reflections: short vertical streaks in the sea below the city
        pg.ink("".join(P([(x, hz + 2.5), (x + .3, hz + 6)]) for x in (264, 268, 272, 276)), 0.6, C["lemon"], 0.7, "1 .8")
    pg.leader((128, 64), (240, 108), dot_a=True, bend=0.1)
    return pg.svg()
