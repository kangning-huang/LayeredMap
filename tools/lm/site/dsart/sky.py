"""Sky and space: meteorites, eclipse, night lights."""
import math

from .kit import (PAL, Art, P, smooth, mix, n, lerp, Ortho, globe, globe_shade, lerp2, cities)


def _unit(v):
    L = math.hypot(*v) or 1
    return (v[0] / L, v[1] / L)


def _rot(v, a):
    c, s = math.cos(a), math.sin(a)
    return (v[0] * c - v[1] * s, v[0] * s + v[1] * c)


# Centre line of the 8 April 2024 total eclipse over land (lon, lat), from NASA SVS maps:
# Mazatlan, Torreon, Eagle Pass, Kerrville, Dallas, Little Rock, Carbondale, Indianapolis,
# Cleveland, Buffalo, Burlington, Houlton, Newfoundland.
TOTALITY_2024 = [(-115.0, 19.6), (-106.4, 23.2), (-103.4, 25.5), (-100.5, 28.7), (-99.1, 30.0), (-96.8, 32.8),
                 (-92.3, 34.7), (-89.2, 37.7), (-86.2, 39.8), (-81.7, 41.5), (-78.9, 42.9), (-73.2, 44.5),
                 (-67.8, 46.1), (-55.5, 48.5), (-48.0, 49.2)]


def eclipse():
    """Sun-Moon-Earth geometry of a total solar eclipse (not to scale): the Moon's converging
    umbra just reaching the Earth, the diverging penumbra, the day and night sides, and the
    2024 path of totality across North America."""
    a = Art("amber", seed=5)
    E = Ortho(-62, 22, 54, (258, 94))
    T, _ = E(-99.5, 29.5)                      # where the umbra touches down (Texas)
    C1, R1 = (-56.0, 74.0), 86.0               # the Sun, mostly off the plate
    u = _unit((T[0] - C1[0], T[1] - C1[1]))
    A = (T[0] + u[0] * 2.5, T[1] + u[1] * 2.5)  # umbra apex just below the surface
    dA = math.hypot(A[0] - C1[0], A[1] - C1[1])
    am = 66.0                                   # Moon to apex
    C2 = (A[0] - u[0] * am, A[1] - u[1] * am)
    R2 = am * R1 / dA                           # the Moon's radius that puts the apex at A

    # ---- the Sun: limb-darkened photosphere and a faint inner corona
    sun_g = a.rad([(0, "#fffaf0"), (.6, a.AP), (.9, a.AL), (1, mix(a.A, a.AL, .4))], .5, .5, .5)
    a.circle(C1, R1 + 7, fill=a.rad([(.86, a.AP, .9), (1, a.AP, 0)]))
    a.circle(C1, R1, fill=sun_g)
    for k in range(7):  # granulation hint near the visible limb
        ang = math.radians(-50 + k * 16)
        p = (C1[0] + (R1 - 8) * math.cos(ang), C1[1] + (R1 - 8) * math.sin(ang))
        a.circle(p, 1.6, fill=a.AL, op=.5)
    a.circle(C1, R1, cls="h", so=.4)

    # ---- tangent geometry
    beta = math.asin((R1 - R2) / math.hypot(C2[0] - C1[0], C2[1] - C1[1]))
    back = (-u[0], -u[1])
    tl = math.sqrt(max(0, (math.hypot(A[0] - C2[0], A[1] - C2[1])) ** 2 - R2 ** 2))
    tls = math.sqrt(max(0, dA ** 2 - R1 ** 2))
    um_pts, sun_pts = [], []
    for s in (1, -1):
        d = _rot(back, s * beta)
        um_pts.append((A[0] + d[0] * tl, A[1] + d[1] * tl))
        sun_pts.append((A[0] + d[0] * tls, A[1] + d[1] * tls))
    dC = math.hypot(C2[0] - C1[0], C2[1] - C1[1])
    X = (C1[0] + u[0] * dC * R1 / (R1 + R2), C1[1] + u[1] * dC * R1 / (R1 + R2))
    gam = math.asin(R1 / math.hypot(X[0] - C1[0], X[1] - C1[1]))
    dXC2 = math.hypot(C2[0] - X[0], C2[1] - X[1])
    tli = math.sqrt(max(0, dXC2 ** 2 - R2 ** 2))
    pen_near, pen_far = [], []
    for s in (1, -1):
        d = _rot(u, s * gam)
        pen_near.append((X[0] + d[0] * tli, X[1] + d[1] * tli))
        pen_far.append((X[0] + d[0] * 250, X[1] + d[1] * 250))

    # rays from the Sun's limb grazing the Moon (hairlines)
    for p, q in zip(sun_pts, um_pts):
        a.path(P([p, q]), stroke=a.AD, sw=.5, so=.55, da="2 2")
    for s, q in zip((1, -1), pen_near):
        d = _rot(u, s * gam)
        a.path(P([(X[0] - d[0] * 130, X[1] - d[1] * 130), q]), stroke=a.AD, sw=.5, so=.4, da="2 2")

    # ---- Earth
    disc, cp = globe(a, E, land=PAL["sand2"], border=False)
    a.group(cp=cp)
    # night side: the hemisphere facing away from the subsolar point (declination +7 in April)
    sub = (-99.5, 7.4)
    anti = (sub[0] + 180, -sub[1])
    a.path(E.cap(anti, 96), fill=PAL["slated"], fo=.12)
    a.path(E.cap(anti, 90), fill=PAL["slated"], fo=.42)
    # penumbral shadow on the ground, deepening towards the path
    for rad_deg, op in ((34, .08), (24, .1), (14, .12), (6, .14)):
        a.path(E.cap((-95, 33), rad_deg), fill=PAL["slated"], fo=op)
    # path of totality
    pts = []
    for (p, q) in zip(TOTALITY_2024, TOTALITY_2024[1:]):
        for k in range(6):
            pts.append((p[0] + (q[0] - p[0]) * k / 6, p[1] + (q[1] - p[1]) * k / 6))
    pts.append(TOTALITY_2024[-1])
    d = E.line(pts)
    a.path(d, fill="none", stroke=a.AP, sw=4.2, lc="round", lj="round", so=.8)
    a.path(d, fill="none", stroke=a.AD, sw=2.0, lc="round", lj="round")
    a.end()
    globe_shade(a, E, disc, op=.22)

    # ---- penumbra and umbra cones in space (translucent, drawn over the Earth they fall on)
    pen = [pen_near[0], pen_far[0], pen_far[1], pen_near[1]]
    a.path(P(pen, True), fill=a.lin([(0, PAL["slated"], .13), (.55, PAL["slated"], .05), (1, PAL["slated"], 0)],
                                      C2[0], C2[1], C2[0] + u[0] * 190, C2[1] + u[1] * 190, user=True))
    a.path(P([pen_near[0], pen_far[0]]) + P([pen_near[1], pen_far[1]]),
           stroke=a.lin([(0, PAL["ink"], .45), (1, PAL["ink"], 0)], C2[0], C2[1], C2[0] + u[0] * 200, C2[1] + u[1] * 200, user=True),
           sw=.55, fill="none")
    um = [um_pts[0], A, um_pts[1]]
    a.path(P(um, True), fill=a.lin([(0, PAL["slated"], .75), (1, PAL["slated"], .55)], 0, 0, 1, 0))
    a.path(P(um), cls="h", so=.7)
    a.circle(T, 2.2, fill=PAL["ink"])

    # ---- the Moon at new phase: lit on the sunward side, dark towards the Earth
    mg = a.lin([(0, "#e8e6e0"), (.48, PAL["rock1"]), (.52, PAL["slate2"]), (1, PAL["slated"])],
               C2[0] - R2 * u[0], C2[1] - R2 * u[1], C2[0] + R2 * u[0], C2[1] + R2 * u[1], user=True)
    a.circle(C2, R2, fill=mg)
    for (dx, dy, r) in ((-.35, -.3, .18), (.1, .35, .14), (-.15, .15, .1)):
        a.circle((C2[0] + dx * R2, C2[1] + dy * R2), r * R2, fill=PAL["ink"], fo=.12)
    a.circle(C2, R2, cls="o", sw=.8)
    # sunlight: parallel rays along the Sun-Earth line
    perp = (-u[1], u[0])
    for k in (0, 1):
        p0 = (C1[0] + u[0] * (R1 + 18) + perp[0] * (62 + 8 * k), C1[1] + u[1] * (R1 + 18) + perp[1] * (62 + 8 * k))
        a.arrow([p0, (p0[0] + u[0] * 46, p0[1] + u[1] * 46)], color=a.AD, w=.9, head=4.4, curve=False)
    return a.render()


def meteorites():
    """A meteoroid's fall: the fireball and ablation trail through the atmosphere, break-up at
    altitude, the fragments' dark flight, and the strewn field on the ground where the largest
    pieces carry farthest downrange; inset, a cut stone with its fusion crust and chondrules."""
    from .kit import box_faces, ob, scallop
    a = Art("amber", seed=111)
    # atmosphere shells over the curve of the Earth
    for k, r_ in enumerate((640, 580, 530)):      # atmospheric layers over the curve of the Earth
        a.circle((160, 690), r_, fill="none", stroke=PAL["water3"], sw=.5, so=.45, da="3 2.4")
    a.circle((160, 690), 530, fill=PAL["sky1"], fo=.5)
    # ---- ground slab with the strewn field
    o = (14, 172)
    Wd, D = 236.0, 84.0
    f, t, r = box_faces(0, Wd, -6, 0, D, o)
    a.path(P(t, True), fill=mix(PAL["sand1"], PAL["sage1"], .4))
    rnd = a.rng
    a.dots([ob(rnd.uniform(2, Wd - 2), rnd.uniform(2, D - 2), 0, o) for _ in range(160)], r=.35, color=PAL["umber"], op=.45)
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)
    ell = [ob(150 + 64 * math.cos(tt), 40 + 18 * math.sin(tt), 0, o) for tt in [2 * math.pi * k / 40 for k in range(40)]]
    a.path(P(ell, True), fill=a.AP, fo=.5, stroke=a.AD, sw=.7, da="2.4 1.6")
    # fragments: small near the uprange end, large downrange
    frags = []
    for k in range(14):
        x = 96 + k * 8.6 + rnd.uniform(-2, 2)
        y = 40 + rnd.uniform(-12, 12) * (1 - abs(k - 7) / 9)
        s = .7 + k * .2
        frags.append((x, y, s))
    for (x, y, s) in frags:
        p = ob(x, y, 0, o)
        a.path(f"M{n(p[0] - s)} {n(p[1])}q{n(s)} {n(-s * 1.4)} {n(2 * s)} 0q{n(-s)} {n(s * .7)} {n(-2 * s)} 0Z", fill=PAL["char"])
    # ---- trajectory: luminous flight, break-up, dark flight
    entry, burst = (6, 16), (128, 70)
    tr = [entry, lerp2(entry, burst, .5), burst]
    a.path(P([entry, (burst[0] - 2, burst[1] - 4), (burst[0] + 2, burst[1] + 4)], True), fill=a.lin([(0, a.AP, 0), (1, a.AL, .9)], 0, 0, 1, 1))
    a.path(P([lerp2(entry, burst, .45), burst]), stroke=a.A, sw=1.6, lc="round")
    a.path(P([entry, burst]), stroke=a.AD, sw=.5, so=.6)
    a.circle(burst, 7, fill=a.rad([(0, "#fffbea"), (.5, a.AL, .9), (1, a.AL, 0)]))
    a.circle(burst, 2.6, fill="#fff", stroke=a.A, sw=.8)
    for (x, y, s) in frags[::2]:
        g = ob(x, y, 0, o)
        mid = (lerp(burst[0], g[0], .55) + 8 + s * 2, lerp(burst[1], g[1], .4))
        a.path(smooth([burst, mid, g]), stroke=PAL["ink"], sw=.5, da="1.6 1.4", fill="none", so=.7)
    # ---- inset: a cut chondrite
    C, R = (262, 56), 34
    a.circle((C[0] + 3, C[1] + 4), R, fill=PAL["ink"], fo=.12)
    stone = scallop([(C[0] + R * .95 * math.cos(tt), C[1] + R * .85 * math.sin(tt)) for tt in [2 * math.pi * k / 9 + .2 for k in range(9)]], bulge=.1, seed=3)
    a.path(stone, fill=PAL["char"])
    inner = scallop([(C[0] + R * .82 * math.cos(tt), C[1] + R * .72 * math.sin(tt)) for tt in [2 * math.pi * k / 9 + .2 for k in range(9)]], bulge=.1, seed=3)
    a.path(inner, fill=a.lin([(0, PAL["rock1"]), (1, PAL["rock"])], 0, 0, 1, 1))
    cp = a.clip(inner)
    a.group(cp=cp)
    for _ in range(22):
        c = (C[0] + rnd.uniform(-R, R), C[1] + rnd.uniform(-R, R))
        rr = rnd.uniform(1.2, 3.6)
        a.circle(c, rr, fill=mix(PAL["rock1"], "#fff", .35), stroke=PAL["rockd"], sw=.4)
    a.dots([(C[0] + rnd.uniform(-R, R), C[1] + rnd.uniform(-R, R)) for _ in range(40)], r=.6, color="#e8e2d0")
    a.end()
    a.path(stone, cls="o", sw=.8)
    # leader from the largest fragment to the inset
    big = ob(frags[-1][0], frags[-1][1], 0, o)
    a.path(P([big, (C[0] - 8, C[1] + R - 2)]), cls="h", da="1.6 1.4", so=.7)
    return a.render()


def night_lights():
    """The Earth from orbit across the day-night line: city lights on the night side, brightest
    where the bundled cities are largest, joined along coasts and rivers, and a polar-orbiting
    satellite's radiometer sweeping a swath across the dark side."""
    a = Art("amber", seed=113)
    E = Ortho(30, 30, 80, (186, 96))
    disc, cp = globe(a, E, land=PAL["sand2"], grat=False)
    a.group(cp=cp)
    sub = (-40, -8)                    # subsolar point: Europe to Asia is in darkness
    anti = (sub[0] + 180, -sub[1])
    a.path(E.cap(anti, 96), fill="#1d2532", fo=.35)
    a.path(E.cap(anti, 90), fill="#1d2532", fo=.78)
    a.add(f'<use href="#{a.land_id}" fill="none" stroke="#8c96a3" stroke-width=".3" stroke-opacity=".5"/>')
    from .kit import ang_dist, small_circle
    glow, core = [], []
    for (lon, lat, pop, _) in cities():
        if ang_dist((lon, lat), anti) > 86:
            continue
        p, v = E(lon, lat, clamp=False)
        if not v:
            continue
        if pop < 2.5e5:
            continue
        s = .5 + 1.9 * min(1, (pop / 1.5e7)) ** .5
        glow.append((p, s))
    for (p, s) in sorted(glow, key=lambda q: q[1]):
        if s >= 1:
            a.circle(p, s * 2.6, fill=a.AL, fo=.22)
    a.dots([p for p, s in glow if s < 1], r=.55, color=a.AL)
    a.dots([p for p, s in glow if s >= 1], r=1.0, color="#fff3d6")
    # the terminator line
    term = small_circle(anti, 90, 120)
    a.path(E.line(term + term[:1], curve=True), stroke=a.AL, sw=.5, so=.6, fill="none", da="2 2")
    a.end()
    globe_shade(a, E, disc, op=.3)
    # satellite with solar panel, above the limb
    s = (48, 34)
    a.path(f"M{n(s[0] - 18)} {n(s[1] - 4)}h12v8h-12Z", fill=PAL["slate2"], stroke=PAL["ink"], sw=.4)
    a.path("".join(f"M{n(s[0] - 18 + k * 3)} {n(s[1] - 4)}v8" for k in range(1, 4)), stroke="#fff", sw=.3, so=.5)
    a.path(f"M{n(s[0] - 6)} {n(s[1])}h2", stroke=PAL["ink"], sw=.6)
    a.path(f"M{n(s[0] - 4)} {n(s[1] - 5)}h10v10h-10Z", fill="#e8e4da", stroke=PAL["ink"], sw=.5)
    a.path(f"M{n(s[0] + 6)} {n(s[1] - 2)}h3v4h-3Z", fill=PAL["slate1"], stroke=PAL["ink"], sw=.4)
    tgt, _ = E(62, 32)
    a.path(P([(s[0] + 4, s[1] + 5), (tgt[0] - 14, tgt[1]), (tgt[0] + 14, tgt[1] + 6), (s[0] + 6, s[1] + 5)], True), fill="#fff", fo=.18,
           stroke=PAL["ink"], sw=.4, so=.5, da="1.6 1.4")
    a.arrow([(s[0] - 10, s[1] + 16), (s[0] + 4, s[1] + 30)], w=.7, head=3.4, curve=False)
    return a.render()
