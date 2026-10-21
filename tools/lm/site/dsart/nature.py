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


def _species_wing(a: Art, pts, fill, veins, spots, flip, cx):
    """One monarch wing (given for the right side), mirrored about x = cx when flip: thick black
    margin, a few bold veins and large white margin spots that stay legible at card size."""
    tf = f"translate({n(2 * cx)} 0) scale(-1 1)" if flip else None
    a.group(tf=tf)
    a.path(smooth(pts, True, t=.45), fill=fill, stroke=PAL["ink"], sw=4, lj="round")
    a.path(veins, stroke=PAL["ink"], sw=1.1, fill="none", lc="round")
    a.dots(spots, r=1.15, color="#fff")
    a.end()


def species():
    """A specimen-style monarch butterfly (dorsal view, bold veins and the white-spotted black
    margins) beside a landscape block where its occurrence records cluster along the river
    meadows, the way a downloaded set of sightings maps out a species' habitat."""
    a = Art("amber", seed=101)
    # ---- landscape block: forest with a meadow corridor along a river, records on the meadow
    o = (10, 166)
    Wd, D = 124.0, 146.0

    def h(x, y):
        return 3 + 4 * math.sin(x / 24) * math.cos(y / 22)
    river = lambda y: 62 + 18 * math.sin(y / 30)

    def cover(x, y):
        d = abs(x - river(y))
        return "water" if d < 4 else "meadow" if d < 22 else "forest"
    cols = {"water": PAL["water3"], "meadow": mix(PAL["sage2"], PAL["sand1"], .35), "forest": PAL["forest"]}
    a.shadow(o[0] + 112, o[1] + 3, 104, 5, op=.16)
    surf = terrain(a, o, Wd, D, h, nx=20, ny=18, cover=cover, colours=cols, base=10, levels=4,
                   strata=[(2.5, PAL["umber"]), (99, PAL["clay"])])
    rnd = a.rng
    pts = []
    while len(pts) < 20:
        y = rnd.uniform(10, D - 8)
        x = river(y) + rnd.choice((-1, 1)) * rnd.uniform(8, 18)
        if 4 < x < Wd - 4 and all(math.hypot(x - u, (y - v) * .6) > 9 for u, v in pts):
            pts.append((x, y))
    pts.sort(key=lambda q: -q[1])                      # back to front
    for x, y in pts:
        a.circle(surf(x, y, .5), 2.4, fill=a.A, stroke="#fff", sw=.7)
    # ---- the butterfly (right half drawn, mirrored)
    cx, cy = 255, 66
    S = .74

    def q(dx, dy):
        return (cx + dx * S, cy + dy * S)
    fw = [q(2, -6), q(18, -34), q(46, -52), q(70, -54), q(72, -44), q(60, -18), q(40, -4), q(4, 2)]
    hw = [q(3, 1), q(34, -4), q(54, 10), q(56, 30), q(40, 46), q(18, 46), q(4, 22)]

    def vein_fan(base, tips):
        return "".join(f"M{n(base[0])} {n(base[1])}Q{n(lerp(base[0], t[0], .5))} {n(lerp(base[1], t[1], .5) + 3)} {n(t[0])} {n(t[1])}"
                       for t in tips)
    fv = vein_fan(q(4, -2), [q(34, -42), q(56, -44), q(58, -26), q(42, -8)])
    hv = vein_fan(q(4, 2), [q(44, 6), q(48, 26), q(30, 40)])
    fspots = [q(67, -46 + k * 7) for k in range(4)]
    hspots = [q(50 - k * 9, 32 + k * 4) for k in range(4)]
    wingfill = a.lin([(0, a.AL), (.6, a.A), (1, a.AD)], 0, 0, 1, 1)
    for flip in (False, True):
        _species_wing(a, hw, wingfill, hv, hspots, flip, cx)
        _species_wing(a, fw, wingfill, fv, fspots, flip, cx)
    # body, head and clubbed antennae
    a.path(f"M{n(cx - 3)} {n(cy - 20)}c-1-6 1-9 3-9s4 3 3 9l-1 30c0 6-4 6-4 0Z", fill=PAL["ink"])
    a.circle((cx, cy - 31), 3, fill=PAL["ink"])
    a.path(f"M{n(cx - 1)} {n(cy - 33)}q-4-9-11-13M{n(cx + 1)} {n(cy - 33)}q4-9 11-13", stroke=PAL["ink"], sw=.8, fill="none")
    a.dots([(cx - 12, cy - 46), (cx + 12, cy - 46)], r=1.1)
    # leader from one record on the block to the specimen
    rec = max(pts, key=lambda q_: q_[0] + .6 * q_[1])
    p = surf(rec[0], rec[1], .5)
    a.path(P([(p[0] + 3, p[1] - 1), (cx - 52 * S - 6, cy + 12)]), cls="h", da="1.8 1.6", so=.75)
    return a.render()


# Two flyways of white storks tagged in Germany, as approximate waypoints (lon, lat): the
# western route over Gibraltar to the Sahel and the eastern route over the Bosporus, the Levant
# and the Nile to East and southern Africa. Storks soar on thermals and avoid long sea crossings.
_WEST = [(10.5, 52.5), (7.5, 50.0), (4.5, 46.5), (1.5, 43.5), (-3.5, 40.0), (-5.6, 36.0), (-6.5, 32.0), (-7.0, 26.0), (-6.0, 19.0), (-4.5, 14.0)]
_EAST = [(12.5, 52.5), (16.5, 49.0), (20.5, 46.5), (25.5, 43.5), (29.0, 41.0), (33.0, 38.5), (36.0, 35.0), (34.5, 31.0), (32.6, 26.0),
         (32.5, 18.0), (33.0, 10.0), (34.0, 2.0), (35.0, -6.0), (32.0, -15.0), (28.0, -24.0)]


def storks():
    """A white stork in flight (white body and coverts, black flight feathers, red bill and legs)
    rising on a thermal, and its kind's two migration flyways between Europe and Africa traced
    as GPS fixes on the globe, detouring by land round the Mediterranean."""
    a = Art("verm", seed=103)
    G = Ortho(14, 20, 76, (232, 92))
    disc, cp = globe(a, G, land=PAL["sand2"], border=False)
    a.group(cp=cp)
    # same-colour hairline over the land closes the antialiasing seams between its polygons
    a.body.append(f'<use href="#{a.land_id}" fill="none" stroke="{PAL["sand2"]}" stroke-width=".5" stroke-linejoin="round"/>')
    rnd = a.rng
    for route in (_WEST, _EAST):
        dense = []
        for (p, q) in zip(route, route[1:]):
            for k in range(4):
                dense.append((lerp(p[0], q[0], k / 4), lerp(p[1], q[1], k / 4)))
        dense.append(route[-1])
        d = G.line(dense, curve=True)
        a.path(d, stroke="#fff", sw=3.4, so=.75, fill="none", lc="round")
        a.path(d, stroke=a.A, sw=1.6, fill="none", lc="round")
        for lo, la in dense[2::4]:
            f_ = G(lo + rnd.uniform(-.5, .5), la + rnd.uniform(-.5, .5))[0]
            a.circle(f_, 1.5, fill="#fff", stroke=a.AD, sw=.6)
    nest, _ = G(11.5, 52.8)
    a.circle(nest, 3.2, fill=a.A, stroke="#fff", sw=1)
    a.end()
    globe_shade(a, G, disc, op=.2)
    # ---- the stork, soaring with wings spread (seen from above), bill leading
    cx, cy = 80, 80
    white = "#fbfaf6"
    a.group(tf="translate(14 18) scale(.82)")
    wing_l = [(cx - 4, cy - 2), (cx - 30, cy - 18), (cx - 58, cy - 26), (cx - 76, cy - 24), (cx - 70, cy - 12), (cx - 40, cy + 2), (cx - 8, cy + 6)]
    wing_r = [(cx + 4, cy - 2), (cx + 30, cy - 20), (cx + 60, cy - 30), (cx + 80, cy - 30), (cx + 74, cy - 16), (cx + 42, cy), (cx + 8, cy + 6)]
    a.path(f"M{n(cx - 2)} {n(cy + 15)}l-2 20M{n(cx + 2)} {n(cy + 15)}l2 20", stroke=a.A, sw=1.6, lc="round")           # legs trailing
    for wing, sgn in ((wing_l, -1), (wing_r, 1)):
        a.path(smooth(wing, True, t=.4), fill=PAL["ink"], stroke=PAL["ink"], sw=.6, lj="round")
        # white coverts on the inner, leading part of the wing
        cov = [wing[0], wing[1], (wing[2][0] - sgn * 6, wing[2][1] + 6), (wing[1][0] + sgn * 2, wing[1][1] + 12), wing[-1]]
        a.path(smooth(cov, True, t=.4), fill=white)
        # separated primary tips ("fingers"), rooted inside the outer edge of the wing tip
        segs = []
        for k in range(4):
            r0 = lerp2(wing[3], wing[4], .1 + .22 * k)
            segs.append(f"M{n(r0[0] - sgn * 5)} {n(r0[1])}l{n(sgn * 12)} {n(-.6 + .5 * k)}")
        a.path("".join(segs), stroke=PAL["ink"], sw=1.7, lc="round")
    neck = f"M{n(cx)} {n(cy - 2)}Q{n(cx + 1)} {n(cy - 14)} {n(cx + 2.2)} {n(cy - 24)}"
    a.path(neck, stroke=PAL["ink"], sw=5.2, lc="round", fill="none")
    a.path(f"M{n(cx - 7)} {n(cy - 2)}c0-6 14-6 14 0l-2 18c-2 6-8 6-10 0Z", fill=white, stroke=PAL["ink"], sw=.8)       # body
    a.path(neck, stroke=white, sw=3.8, lc="round", fill="none")                                                        # neck
    a.path(f"M{n(cx + 1)} {n(cy - 27)}L{n(cx + 2.2)} {n(cy - 41)}L{n(cx + 3.4)} {n(cy - 27)}Z", fill=a.A, stroke=a.AD, sw=.5, lj="round")  # bill
    a.circle((cx + 2.2, cy - 25), 3, fill=white, stroke=PAL["ink"], sw=.8)                                              # head
    a.end()
    # ---- the thermal it rides: warm air rising from the ground beneath it
    for s in (-1, 1):
        x0 = 80 + s * 26
        a.arrow([(x0, 170), (x0 + s * 8, 152), (x0 - s * 2, 136), (x0 + s * 4, 120)], color=PAL["ink"], w=1, head=4.6, da="2.6 1.8", op=.7)
    return a.render()


def street_trees():
    """A street tree in section, as recorded in a city tree census: the crown, the trunk with its
    diameter measured at breast height (the tape and the 1.37 m height mark), the tree pit in the
    sidewalk with the shallow, wide root plate spreading under the paving, and the kerb and
    roadway with a parked car for scale."""
    a = Art("verm", seed=105)
    gy = 122                                   # sidewalk surface
    ry = gy + 6                                # roadway surface, below the kerb top
    kx = 206                                   # kerb face
    # ---- soil section: two broad horizons
    a.path(f"M8 {gy}H{kx + 6}V{ry}H312V172H8Z", fill=mix(PAL["soil"], PAL["sand2"], .35))
    a.path(f"M8 {gy + 22}H312V172H8Z", fill=mix(PAL["soil"], PAL["clay"], .5))
    # paving slabs over a base course; kerb; asphalt roadway
    a.path(f"M8 {gy}H{kx}V{gy + 5}H8Z", fill=PAL["rock1"])
    a.path("".join(f"M{x} {gy}v5" for x in (30, 58, 160, 188)), stroke=PAL["ink"], sw=.5, so=.45)
    a.path(f"M{kx + 6} {ry}H312V{ry + 7}H{kx + 6}Z", fill=PAL["slate2"])
    a.path(f"M{kx} {gy}h6V{ry + 12}h-6Z", fill="#ece7dc", stroke=PAL["ink"], sw=.6)
    a.path(f"M8 {gy}H{kx + 6}M{kx + 6} {ry}H312", cls="o", sw=.9)
    # ---- the tree: roots (wide and shallow), pit, trunk and limbs, crown
    tx = 108
    roots = []
    for k, (dx, dy) in enumerate(((-88, 5), (-62, 14), (-28, 19), (30, 19), (64, 14), (92, 6))):
        ex, ey = tx + dx, gy + 6 + dy
        roots.append(f"M{tx + dx * .06:.1f} {gy + 6}Q{tx + dx * .45:.1f} {gy + 7 + dy * .25:.1f} {ex} {ey}")
    for (x0, y0, dx, dy) in ((tx - 46, gy + 13, -7, 6), (tx + 44, gy + 13, 8, 6), (tx - 4, gy + 7, -2, 10), (tx + 4, gy + 7, 3, 9)):
        roots.append(f"M{x0} {y0}l{dx} {dy}")
    a.path("".join(roots), stroke=mix(PAL["umber"], PAL["ink"], .15), sw=1.8, fill="none", lc="round")
    a.path(f"M{tx - 14} {gy}h28v7h-28Z", fill=PAL["umber"])                                      # pit soil
    a.path(f"M{tx - 14} {gy - .8}h28", stroke=PAL["slated"], sw=1.6)                              # grate
    trunk = (f"M{tx - 6} {gy}C{tx - 5} {gy - 24} {tx - 5} {gy - 40} {tx - 16} {gy - 60}L{tx - 9} {gy - 62}"
             f"L{tx} {gy - 48}L{tx + 9} {gy - 66}L{tx + 15} {gy - 61}C{tx + 6} {gy - 42} {tx + 5} {gy - 24} {tx + 6} {gy}Z")
    a.path(trunk, fill=a.lin([(0, PAL["clay"]), (.45, PAL["umber"]), (1, mix(PAL["umber"], PAL["ink"], .35))], 0, 0, 1, 0),
           stroke=PAL["ink"], sw=.6)
    crown = [(tx - 60, 66), (tx - 60, 40), (tx - 42, 18), (tx - 14, 9), (tx + 18, 9), (tx + 46, 20), (tx + 62, 42), (tx + 58, 66),
             (tx + 36, 80), (tx + 6, 83), (tx - 30, 80)]
    cp = scallop(crown, bulge=.3, seed=7)
    a.path(cp, fill=a.lin([(0, PAL["sage2"]), (.5, PAL["moss"]), (1, PAL["forest"])], 0, 0, .6, 1))
    for (cx, cy, r) in ((tx - 30, 36, 17), (tx + 8, 24, 17)):                                    # lit lobes, top left
        a.path(scallop([(cx + r * math.cos(t), cy + r * .8 * math.sin(t)) for t in [math.radians(v) for v in range(200, 341, 35)]],
                       bulge=.25, seed=int(cx)), fill="none", stroke="#fff", sw=1, so=.5)
    a.path(cp, cls="o", sw=.8, so=.7)
    # ---- DBH: tape round the trunk at breast height and the height mark from the ground
    bh = gy - 18
    a.path(f"M{tx - 6.2} {bh - .6}q6.2 3 12.4 0", stroke=a.A, sw=2.2, fill="none", lc="round")
    dx = tx + 20
    a.path(f"M{tx + 9} {bh}h{dx - tx - 5}", stroke=a.AD, sw=.6)
    a.arrow([(dx, gy - 1), (dx, bh + 1)], color=a.AD, w=1, head=3.6, both=True, curve=False)
    # ---- a parked car on the roadway for scale (wheels resting on the asphalt)
    cx = 264
    wr = 5
    a.path(f"M{cx - 30} {ry - 3}v-8l8-10h28l10 10h12v8Z", fill=PAL["water3"], stroke=PAL["ink"], sw=.6)
    a.path(f"M{cx - 19} {ry - 13}l5-6h11v6ZM{cx + 1} {ry - 13}v-6h9l6 6Z", fill=PAL["water1"], stroke=PAL["ink"], sw=.4)
    for wx in (cx - 17, cx + 17):
        a.circle((wx, ry - wr), wr, fill=PAL["slated"], stroke=PAL["ink"], sw=.5)
        a.circle((wx, ry - wr), 2, fill=PAL["rock1"])
    return a.render()


def forest_loss():
    """Tropical forest cleared along roads in the fishbone pattern: a few broad clearings branch
    off the main road into intact forest, older loss pale near the roads and recent loss (the
    accent) at the advancing tips, in the stepped edges of 30 m map pixels; the cut face shows
    the thin soil under the forest."""
    a = Art("verm", seed=107)
    rnd = a.rng
    o = (14, 150)
    Wd, D = 178.0, 148.0
    a.shadow(o[0] + 150, o[1] + 4, 140, 7, op=.2)
    surf = terrain(a, o, Wd, D, lambda x, y: 4.0, nx=4, ny=4, base=14, levels=4,
                   colours={"land": PAL["forest"]}, strata=[(2.5, PAL["umber"]), (99, PAL["clay"])])
    main = lambda x: 72 + x * .05            # main road along x (as a depth position)
    old, new = mix(a.AP, a.AL, .45), a.A
    step = 8.0
    # side roads (the fishbone ribs): x position, reach behind (+) / in front (-) of the main road
    ribs = [(26, 52), (50, -44), (78, 60), (104, -50), (132, 46), (158, -40)]
    clear = []                               # (x0, x1, y0, y1) cleared rectangles, for the canopy
    for sx, reach in ribs:
        y0, sgn, n_ = main(sx), (1 if reach > 0 else -1), max(2, round(abs(reach) / step))
        for k in range(n_):
            hw = 7 + 4 * rnd.random() if k < n_ - 1 else 5 + 3 * rnd.random()
            ya, yb = y0 + sgn * k * step, y0 + sgn * (k + 1) * step
            off = rnd.uniform(-1.5, 1.5)
            rect = (sx - hw + off, sx + hw + off, min(ya, yb), max(ya, yb))
            clear.append(rect)
            a.path(P([surf(x, y, .1) for x, y in [(rect[0], rect[2]), (rect[0], rect[3]), (rect[1], rect[3]), (rect[1], rect[2])]], True),
                   fill=new if k >= n_ * .6 else old, stroke=new if k >= n_ * .6 else old, sw=.3)
    # the cleared corridor along the main road
    xs = [k * Wd / 8 for k in range(9)]
    band = [(x, main(x) - 6) for x in xs] + [(x, main(x) + 6) for x in reversed(xs)]
    a.path(P([surf(x, y, .1) for x, y in band], True), fill=old)
    clear.append((0, Wd, main(0) - 6, main(Wd) + 6))
    # canopy: sparse crowns on the standing forest only
    crowns = []
    for j in range(30, 0, -1):
        for i in range(40):
            x, y = (i + .5 + rnd.uniform(-.3, .3)) * Wd / 40, (j - .5 + rnd.uniform(-.3, .3)) * D / 30
            if 4 < x < Wd - 4 and 4 < y < D - 6 and rnd.random() < .5 \
                    and not any(r[0] - 3 < x < r[1] + 3 and r[2] - 3 < y < r[3] + 3 for r in clear):
                crowns.append(surf(x, y, .6))
    a.dots(crowns, r=1.3, color=PAL["forestd"], op=.3)
    a.dots([(p[0] - .4, p[1] - .5) for p in crowns], r=.6, color=PAL["moss"], op=.45)
    # roads (dirt)
    road = dict(stroke=PAL["umber"], fill="none", lj="round", lc="round")
    a.path(P([surf(x, main(x), .2) for x in (0, Wd)]), sw=1.8, **road)
    for sx, reach in ribs:
        a.path(P([surf(sx, main(sx), .2), surf(sx, main(sx) + reach * .85, .2)]), sw=1.1, **road)
    return a.render()
