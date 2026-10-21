"""Nature and wildlife: species sightings, white storks, street trees, forest loss."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, box_faces, terrain, broadleaf, conifer, scallop,
                  ring_pts, Ortho, globe, globe_shade)


def _wing(a: Art, pts, fill, veins, spots, flip, cx):
    """Draw one wing (given for the right side) mirrored about x = cx when flip."""
    tf = f"translate({n(2 * cx)} 0) scale(-1 1)" if flip else None
    a.group(tf=tf)
    a.path(smooth(pts, True, t=.45), fill=fill, stroke=PAL["ink"], sw=3.2, lj="round")
    a.path(veins, stroke=PAL["ink"], sw=.8, fill="none", lc="round")
    a.dots(spots, r=.75, color="#fff")
    a.end()


def species():
    """A specimen-style monarch butterfly (dorsal view, with wing venation and the white-spotted
    black margins) beside a landscape block where its occurrence records cluster along the
    habitat, the way a downloaded set of sightings maps out a species' range."""
    a = Art("amber", seed=101)
    # ---- landscape block with habitat and occurrence points
    o = (8, 164)
    Wd, D = 112.0, 130.0

    def h(x, y):
        return 3 + 5 * math.sin(x / 21) * math.cos(y / 19)
    river = lambda y: 60 + 22 * math.sin(y / 26)

    def cover(x, y):
        d = abs(x - river(y))
        if d < 4:
            return "water"
        if d < 22:
            return "meadow"
        return "forest" if math.sin(x / 13) * math.cos(y / 17) + .4 * math.sin((x + y) / 7) > -.1 else "field"
    cols = {"water": PAL["water3"], "meadow": mix(PAL["sage2"], PAL["sand1"], .2), "forest": PAL["forest"], "field": PAL["sand1"]}
    a.shadow(o[0] + 110, o[1] + 4, 110, 6, op=.18)
    surf = terrain(a, o, Wd, D, h, nx=20, ny=16, cover=cover, colours=cols, base=10,
                   strata=[(2.5, PAL["umber"]), (99, PAL["clay"])])
    rnd = a.rng
    pts = []
    for _ in range(46):
        y = rnd.uniform(4, D - 4)
        x = river(y) + rnd.gauss(0, 9)
        if 2 < x < Wd - 2:
            pts.append(surf(x, y, .5))
    for p in sorted(pts, key=lambda p: p[1]):
        a.circle(p, 1.5, fill=a.A, stroke="#fff", sw=.5)
    # ---- the butterfly (right half drawn, mirrored)
    cx, cy = 240, 90
    fw = [(cx + 2, cy - 6), (cx + 18, cy - 34), (cx + 46, cy - 52), (cx + 70, cy - 54), (cx + 72, cy - 44), (cx + 60, cy - 18),
          (cx + 40, cy - 4), (cx + 4, cy + 2)]
    hw = [(cx + 3, cy + 1), (cx + 34, cy - 4), (cx + 54, cy + 10), (cx + 56, cy + 30), (cx + 40, cy + 46), (cx + 18, cy + 46), (cx + 4, cy + 22)]
    def vein_fan(base, tips):
        return "".join(f"M{n(base[0])} {n(base[1])}Q{n(lerp(base[0], t[0], .5))} {n(lerp(base[1], t[1], .5) + 4)} {n(t[0])} {n(t[1])}" for t in tips)
    fv = vein_fan((cx + 4, cy - 2), [(cx + 30, cy - 44), (cx + 50, cy - 50), (cx + 64, cy - 50), (cx + 68, cy - 38), (cx + 60, cy - 22), (cx + 46, cy - 8)])
    fv += f"M{n(cx + 8)} {n(cy - 8)}L{n(cx + 34)} {n(cy - 22)}L{n(cx + 52)} {n(cy - 30)}"
    hv = vein_fan((cx + 4, cy + 2), [(cx + 34, cy - 2), (cx + 50, cy + 10), (cx + 54, cy + 26), (cx + 44, cy + 40), (cx + 30, cy + 46), (cx + 16, cy + 44)])
    hv += f"M{n(cx + 10)} {n(cy + 6)}Q{n(cx + 30)} {n(cy + 4)} {n(cx + 38)} {n(cy + 22)}Q{n(cx + 28)} {n(cy + 30)} {n(cx + 12)} {n(cy + 18)}"
    fspots = [(cx + 62 + rnd.uniform(-1, 1), cy - 48 + k * 4.4) for k in range(7)] + [(cx + 52 - k * 4, cy - 12 + k * 1.6) for k in range(4)]
    hspots = [(cx + 52 - k * 4.4, cy + 30 + k * 3.2) for k in range(7)] + [(cx + 48 - k * 3.8, cy + 36 + k * 2.4) for k in range(5)]
    wingfill = a.lin([(0, a.AL), (.6, a.A), (1, a.AD)], 0, 0, 1, 1)
    for flip in (False, True):
        _wing(a, hw, wingfill, hv, hspots, flip, cx)
        _wing(a, fw, wingfill, fv, fspots, flip, cx)
    # black outer margins (drawn as thick inner strokes clipped to the wing)
    a.path(f"M{n(cx - 3)} {n(cy - 22)}c-1-6 1-10 3-10s4 4 3 10l-1 34c0 6-4 6-4 0Z", fill=PAL["ink"])
    a.circle((cx, cy - 33), 3, fill=PAL["ink"])
    a.path(f"M{n(cx - 1)} {n(cy - 35)}q-5-10-12-14M{n(cx + 1)} {n(cy - 35)}q5-10 12-14", stroke=PAL["ink"], sw=.7, fill="none")
    a.dots([(cx - 1.2, cy - 22 + k * 5) for k in range(5)] + [(cx + 1.2, cy - 19.5 + k * 5) for k in range(5)], r=.45, color="#fff")
    # leader from one record to the specimen
    p = pts[len(pts) // 2]
    a.path(P([p, (cx - 70, cy + 40)]), cls="h", da="1.6 1.4", so=.7)
    return a.render()


# Two flyways of white storks tagged in Germany, as approximate waypoints (lon, lat): the
# western route over Gibraltar to the Sahel and the eastern route over the Bosporus, the Levant
# and the Nile to East and southern Africa. Storks soar on thermals and avoid long sea crossings.
_WEST = [(10.5, 52.5), (7.5, 50.0), (4.5, 46.5), (1.5, 43.5), (-3.5, 40.0), (-5.6, 36.0), (-6.5, 32.0), (-7.0, 26.0), (-6.0, 19.0), (-4.5, 14.0)]
_EAST = [(12.5, 52.5), (16.5, 49.0), (20.5, 46.5), (25.5, 43.5), (29.0, 41.0), (33.0, 38.5), (36.0, 35.0), (34.5, 31.0), (32.6, 26.0),
         (32.5, 18.0), (33.0, 10.0), (34.0, 2.0), (35.0, -6.0), (32.0, -15.0), (28.0, -24.0)]


def storks():
    """A white stork in flight (white body and coverts, black flight feathers, red bill and legs)
    and its kind's two migration flyways between Europe and Africa traced as GPS fixes on the
    globe, detouring by land round the Mediterranean."""
    a = Art("verm", seed=103)
    G = Ortho(14, 20, 76, (232, 92))
    disc, cp = globe(a, G, land=PAL["sand1"], border=True)
    a.group(cp=cp)
    rnd = a.rng
    for route, w in ((_WEST, 1.1), (_EAST, 1.1)):
        dense = []
        for (p, q) in zip(route, route[1:]):
            for k in range(5):
                dense.append((lerp(p[0], q[0], k / 5), lerp(p[1], q[1], k / 5)))
        dense.append(route[-1])
        d = G.line(dense, curve=True)
        a.path(d, stroke="#fff", sw=2.6, so=.7, fill="none", lc="round")
        a.path(d, stroke=a.A, sw=w, fill="none", lc="round")
        fixes = [G(lo + rnd.uniform(-.8, .8), la + rnd.uniform(-.8, .8))[0] for lo, la in dense[::2]]
        for f_ in fixes:
            a.circle(f_, .9, fill="#fff", stroke=a.AD, sw=.45)
    nest, _ = G(11.5, 52.8)
    a.circle(nest, 2.6, fill=a.A, stroke="#fff", sw=.8)
    a.end()
    globe_shade(a, G, disc, op=.2)
    # ---- the stork, gliding with wings spread (seen from above-front)
    cx, cy = 80, 80
    a.group(tf="translate(14 18) scale(.82)")
    wing_l = [(cx - 4, cy - 2), (cx - 30, cy - 18), (cx - 58, cy - 26), (cx - 76, cy - 24), (cx - 70, cy - 12), (cx - 40, cy + 2), (cx - 8, cy + 6)]
    wing_r = [(cx + 4, cy - 2), (cx + 30, cy - 20), (cx + 60, cy - 30), (cx + 80, cy - 30), (cx + 74, cy - 16), (cx + 42, cy), (cx + 8, cy + 6)]
    for wing, sgn in ((wing_l, -1), (wing_r, 1)):
        a.path(smooth(wing, True, t=.4), fill=PAL["ink"], stroke=PAL["ink"], sw=.6)
        # white coverts on the inner, leading part of the wing
        cov = [wing[0], wing[1], (wing[2][0] - sgn * 6, wing[2][1] + 6), (wing[1][0] + sgn * 2, wing[1][1] + 12), wing[-1]]
        a.path(smooth(cov, True, t=.4), fill="#fbfaf6")
        # separated primary tips ("fingers")
        tip = wing[3]
        a.path("".join(f"M{n(tip[0] - sgn * 6 + sgn * k * 1.6)} {n(tip[1] + 2 + k * 2.2)}l{n(sgn * 7)} {n(-1 + k * .4)}" for k in range(5)),
               stroke=PAL["ink"], sw=1.0, lc="round")
        a.path("".join(f"M{n(wing[1][0] + sgn * k * 6)} {n(wing[1][1] + 10 + k * .3)}l{n(sgn * 4)} 3" for k in range(6)),
               stroke="#fff", sw=.35, so=.5)
    a.path(f"M{n(cx - 7)} {n(cy - 2)}c0-6 14-6 14 0l-2 18c-2 6-8 6-10 0Z", fill="#fbfaf6", stroke=PAL["ink"], sw=.6)   # body
    a.path(f"M{n(cx - 2)} {n(cy - 6)}q1-12 4-18", stroke="#fbfaf6", sw=3.6, lc="round", fill="none")                     # neck
    a.circle((cx + 2.2, cy - 24.5), 2.6, fill="#fbfaf6", stroke=PAL["ink"], sw=.5)
    a.path(f"M{n(cx + 3)} {n(cy - 27)}l1.4-13 1.2 13Z", fill=a.A, stroke=a.AD, sw=.4)                                    # bill
    a.path(f"M{n(cx - 2)} {n(cy + 15)}l-2 20M{n(cx + 2)} {n(cy + 15)}l2 20", stroke=a.A, sw=1.1, lc="round")             # legs trailing
    # thermal it is soaring on
    a.end()
    cx, cy = 80, 84
    a.arrow([(cx - 20, 168), (cx - 30, 150), (cx - 18, 134), (cx - 30, 116)], color=PAL["ink"], w=.7, head=3.6, da="2 1.6", op=.6)
    a.arrow([(cx + 22, 168), (cx + 32, 150), (cx + 20, 134), (cx + 32, 116)], color=PAL["ink"], w=.7, head=3.6, da="2 1.6", op=.6)
    return a.render()


def street_trees():
    """A street tree in section: crown, trunk with its diameter measured at breast height (1.37 m,
    as in the census), the tree pit and grate in the sidewalk, roots spreading under the paving
    among utility pipes, the kerb and the roadway, a building facade behind."""
    a = Art("verm", seed=105)
    gy = 120
    # ---- soil section below ground
    a.path(f"M8 {gy}H312V172H8Z", fill=a.lin([(0, PAL["soil"]), (1, PAL["clay"])], 0, 0, 0, 1))
    a.path(f"M8 {gy}H312V172H8Z", fill=a.pattern(6, 5, f'<circle cx="1" cy="1" r=".45" fill="{mix(PAL["soil"], PAL["ink"], .4)}"/><circle cx="4" cy="3.4" r=".35" fill="{mix(PAL["soil"], PAL["ink"], .4)}"/>'))
    # paving base course and slabs; roadway asphalt layers at the right
    a.path(f"M8 {gy}H210V{gy + 6}H8Z", fill=PAL["rock1"])
    a.path("".join(f"M{x} {gy}v4" for x in range(20, 210, 14)), stroke=PAL["ink"], sw=.35, so=.5)
    a.path(f"M216 {gy + 4}H312V{gy + 12}H216Z", fill=PAL["slate2"])
    a.path(f"M216 {gy + 12}H312V{gy + 18}H216Z", fill=PAL["rock"])
    a.path(f"M210 {gy - 3}h6V{gy + 14}h-6Z", fill="#ece7dc", stroke=PAL["ink"], sw=.5)            # kerb
    a.path(f"M8 {gy}H210M210 {gy - 3}h6M216 {gy + 4}H312", cls="o", sw=.9)
    # utilities: water main and a duct bank
    a.circle((176, 150), 5, fill=PAL["water2"], stroke=PAL["ink"], sw=.6)
    a.circle((176, 150), 3.4, fill="none", stroke=PAL["ink"], sw=.3)
    for k in range(3):
        a.circle((40 + k * 7, 160), 2.6, fill=PAL["rock1"], stroke=PAL["ink"], sw=.4)
    # ---- building facade at the left
    a.path(f"M8 {gy}V8H52V{gy}Z", fill=a.lin([(0, "#efe9dd"), (1, PAL["sand2"])], 0, 0, 1, 0), stroke=PAL["ink"], sw=.6)
    for r in range(4):
        for c in range(2):
            a.path(f"M{16 + c * 18} {18 + r * 24}h10v14h-10Z", fill=PAL["water1"], stroke=PAL["ink"], sw=.4)
    a.path(f"M22 {gy}v-18h14v18", fill=PAL["slate1"], stroke=PAL["ink"], sw=.5)
    # ---- the tree: roots, pit, trunk, crown
    tx = 130
    roots = ""
    rnd = a.rng
    for k in range(9):
        ang = math.radians(15 + k * 18.75)
        L = 34 + rnd.uniform(-6, 10)
        ex, ey = tx + math.cos(ang) * L * 1.8, gy + 6 + math.sin(ang) * L * .45
        roots += f"M{tx} {gy + 4}Q{n(lerp(tx, ex, .5))} {n(gy + 8 + (ey - gy) * .2)} {n(ex)} {n(ey)}"
        for j in range(2):
            t = .5 + j * .25
            bx, by = lerp(tx, ex, t), lerp(gy + 6, ey, t)
            roots += f"M{n(bx)} {n(by)}l{n(rnd.uniform(-6, 6))} {n(rnd.uniform(3, 7))}"
    a.path(roots, stroke=PAL["umber"], sw=1.1, fill="none", lc="round")
    a.path(roots, stroke=PAL["ink"], sw=.3, fill="none", so=.5)
    a.path(f"M{tx - 16} {gy}h32v8h-32Z", fill=PAL["umber"])                                    # pit soil
    a.path("".join(f"M{tx - 15 + k * 3} {gy - 1}h1.6" for k in range(11)), stroke=PAL["slated"], sw=1.6)   # grate
    trunk = f"M{tx - 5} {gy}C{tx - 4} {gy - 30} {tx - 4} {gy - 46} {tx - 10} {gy - 64}L{tx + 2} {gy - 58}L{tx + 12} {gy - 70}C{tx + 5} {gy - 48} {tx + 4} {gy - 30} {tx + 5} {gy}Z"
    a.path(trunk, fill=a.lin([(0, PAL["umber"]), (.4, PAL["clay"]), (1, mix(PAL["umber"], PAL["ink"], .3))], 0, 0, 1, 0), stroke=PAL["ink"], sw=.6)
    a.path("".join(f"M{tx - 3 + k * 2} {gy - 4}v-{20 + k * 3}" for k in range(4)), stroke=PAL["ink"], sw=.3, so=.35)
    crown = [(tx - 64, 72), (tx - 66, 46), (tx - 48, 22), (tx - 20, 10), (tx + 14, 8), (tx + 46, 18), (tx + 66, 40), (tx + 66, 66),
             (tx + 44, 82), (tx + 10, 86), (tx - 30, 86)]
    cp = scallop(crown, bulge=.3, seed=7)
    a.path(cp, fill=a.lin([(0, PAL["sage2"]), (.5, PAL["moss"]), (1, PAL["forest"])], 0, 0, .6, 1))
    for (cx, cy, r) in ((tx - 34, 40, 16), (tx + 6, 28, 18), (tx + 38, 46, 15), (tx - 12, 62, 16), (tx + 30, 70, 12)):
        a.path(scallop([(cx + r * math.cos(t), cy + r * .8 * math.sin(t)) for t in [math.radians(v) for v in range(200, 341, 28)]], bulge=.25, seed=int(cx)),
               fill="none", stroke="#fff", sw=.7, so=.45)
    a.path(cp, cls="h", so=.7)
    a.path(f"M{tx - 2} {gy - 40}l-14-20M{tx + 2} {gy - 46}l12-18", stroke=PAL["umber"], sw=1.8, lc="round")
    # ---- DBH: tape round the trunk at breast height and its height dimension
    bh = gy - 26
    a.path(f"M{tx - 5.4} {bh}q5.4 2.2 10.8 0", stroke=a.A, sw=1.4, fill="none")
    a.arrow([(tx + 18, gy - 1), (tx + 18, bh + 1)], color=a.AD, w=.8, head=3.4, both=True, curve=False)
    a.path(f"M{tx + 7} {bh}h14M{tx + 14} {gy}h8", stroke=a.AD, sw=.5)
    a.arrow([(tx - 18, bh), (tx - 7, bh)], color=a.AD, w=.7, head=3, curve=False)
    # a parked car on the roadway for scale
    cx = 268
    a.path(f"M{cx - 30} {gy + 4}v-8l8-10h30l10 10h10v8Z", fill=PAL["water3"], stroke=PAL["ink"], sw=.6)
    a.path(f"M{cx - 18} {gy - 12}l4-6h12v6ZM{cx + 2} {gy - 12}v-6h10l6 6Z", fill=PAL["water1"], stroke=PAL["ink"], sw=.4)
    for wx in (cx - 18, cx + 18):
        a.circle((wx, gy + 4), 5, fill=PAL["slated"], stroke=PAL["ink"], sw=.5)
        a.circle((wx, gy + 4), 2, fill=PAL["rock1"])
    return a.render()


def forest_loss():
    """Tropical forest cleared along roads in the fishbone pattern, mapped as 30 m pixels coloured
    by the year each was lost (pale for early years, dark for recent), as in the Hansen
    lossyear layer; the cut face shows the thin soil under the forest."""
    a = Art("verm", seed=107)
    o = (10, 162)
    Wd, D = 200.0, 150.0

    def h(x, y):
        return 5 + 3 * math.sin(x / 23) * math.cos(y / 19) + 2 * math.sin(y / 11)
    main = lambda x: 70 + x * .12          # main road along x (as a depth position)
    sides = [30, 62, 96, 130, 166]          # side roads along depth at these x

    def year(x, y):
        dm = abs(y - main(x))
        best = None
        if dm < 30:
            best = 1 + int(dm / 7)
        for k, sx in enumerate(sides):
            ds = abs(x - sx)
            if ds < 14 and abs(y - main(x)) < 70 - k * 4:
                yr = 2 + int(abs(y - main(x)) / 16) + int(ds / 7)
                best = yr if best is None else min(best, yr)
        return best

    def cover(x, y):
        yr = year(x, y)
        if yr is None or yr > 6:
            return "forest"
        return f"y{yr}"
    cols = {"forest": PAL["forest"]}
    for k in range(1, 7):
        cols[f"y{k}"] = mix(a.AP, a.AD, (k - 1) / 5)
    a.shadow(o[0] + 150, o[1] + 4, 150, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=32, ny=24, cover=cover, colours=cols, base=10, levels=4,
                   strata=[(2, PAL["umber"]), (5, mix(PAL["clay"], a.AL, .3)), (99, PAL["clay"])])
    # canopy texture on the remaining forest
    rnd = a.rng
    crowns = []
    for j in range(36, 0, -1):
        for i in range(48):
            x, y = (i + .5 + rnd.uniform(-.3, .3)) * Wd / 48, (j - .5 + rnd.uniform(-.3, .3)) * D / 36
            if 1 < x < Wd - 1 and 1 < y < D - 1 and cover(x, y) == "forest" and rnd.random() < .45:
                crowns.append(surf(x, y))
    a.dots(crowns, r=1.25, color=PAL["forestd"], op=.55)
    a.dots([(p[0] - .5, p[1] - .6) for p in crowns[::2]], r=.6, color=PAL["moss"], op=.8)
    # roads
    a.path(smooth([surf(x, main(x), .3) for x in range(0, int(Wd) + 1, 20)]), stroke="#efe6d4", sw=1.4, fill="none")
    for k, sx in enumerate(sides):
        lim = 70 - k * 4
        a.path(P([surf(sx, max(1, main(sx) - lim), .3), surf(sx, min(D - 1, main(sx) + lim), .3)]), stroke="#efe6d4", sw=.8, fill="none")
    # pixel grid hint over a cleared patch
    g = []
    for i in range(6):
        g.append(P([surf(56 + i * 5, 40, .4), surf(56 + i * 5, 65, .4)]))
        g.append(P([surf(56, 40 + i * 5, .4), surf(81, 40 + i * 5, .4)]))
    a.path("".join(g), stroke="#fff", sw=.3, so=.6, fill="none")
    # legend-free year ramp as a small graduated bar (no numbers): early to recent
    for k in range(6):
        x = 220 + k * 13
        a.path(f"M{x} 24h12v7h-12Z", fill=cols[f"y{k + 1}"], stroke=PAL["ink"], sw=.35)
    a.arrow([(220, 37), (296, 37)], w=.7, head=3.4, curve=False)
    return a.render()
