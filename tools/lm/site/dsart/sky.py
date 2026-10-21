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
    """Sun-Moon-Earth geometry of a total solar eclipse (not to scale): sunlight grazing the
    Moon's limb converges into the umbra that just reaches the Earth, the faint diverging
    penumbra, the night side, and the 2024 path of totality across North America."""
    a = Art("amber", seed=5)
    E = Ortho(-70, 24, 55, (258, 92))
    T, _ = E(-99.5, 29.5)                      # where the umbra touches down (Texas)
    C1, R1 = (-16.0, 90.0), 70.0               # the Sun: a deliberate partial disc on the left edge
    u = _unit((T[0] - C1[0], T[1] - C1[1]))
    A = (T[0] + u[0] * 2.5, T[1] + u[1] * 2.5)  # umbra apex just below the surface
    dA = math.hypot(A[0] - C1[0], A[1] - C1[1])
    am = 74.0                                   # Moon to apex
    C2 = (A[0] - u[0] * am, A[1] - u[1] * am)
    R2 = am * R1 / dA                           # the Moon's radius that puts the apex at A

    # ---- the Sun: soft glow and a limb-darkened disc
    a.circle(C1, R1 + 14, fill=a.rad([(.8, a.AP, .75), (.9, a.AP, .25), (1, a.AP, 0)]))
    a.circle(C1, R1, fill=a.rad([(0, "#fffaf0"), (.65, a.AP), (1, a.AL)], .5, .5, .5))
    a.circle(C1, R1, cls="h", so=.4)

    # ---- tangent geometry (outer tangents -> umbra, inner tangents -> penumbra)
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

    # ---- sunlight from the two limbs grazing the Moon: the rays that build the umbra
    for s0, m0 in zip(sun_pts, um_pts):
        a.path(P([s0, m0]), stroke=a.AD, sw=.8, fill="none", so=.75)
        mid = lerp2(s0, m0, .5)
        a.arrow([lerp2(s0, m0, .44), mid], color=a.AD, w=.8, head=4.2, curve=False)

    # ---- penumbra cone in space: a faint wash, fading with distance
    pen = [pen_near[0], pen_far[0], pen_far[1], pen_near[1]]
    a.path(P(pen, True), fill=a.lin([(0, PAL["slated"], .06), (.6, PAL["slated"], .02), (1, PAL["slated"], 0)],
                                      C2[0], C2[1], C2[0] + u[0] * 200, C2[1] + u[1] * 200, user=True))

    # ---- Earth: land, night side and the path of totality
    disc, cp = globe(a, E, land=PAL["sand2"], grat=False, border=False)
    a.group(cp=cp)
    sub = (-99.5, 7.4)                          # subsolar point, declination +7 in April
    a.path(E.cap((sub[0] + 180, -sub[1]), 90), fill=PAL["slated"], fo=.32)
    pts = []
    for (p, q) in zip(TOTALITY_2024, TOTALITY_2024[1:]):
        for k in range(6):
            pts.append((p[0] + (q[0] - p[0]) * k / 6, p[1] + (q[1] - p[1]) * k / 6))
    pts.append(TOTALITY_2024[-1])
    d = E.line(pts)
    a.path(d, fill="none", stroke="#ffffff", sw=7, lc="round", lj="round", so=.85)
    a.path(d, fill="none", stroke=a.AP, sw=4.4, lc="round", lj="round")
    a.path(d, fill="none", stroke=a.AD, sw=1.8, lc="round", lj="round")
    a.end()
    globe_shade(a, E, disc, op=.22)

    # ---- penumbra edges, then the umbra touching down
    a.path(P([pen_near[0], pen_far[0]]) + P([pen_near[1], pen_far[1]]),
           stroke=a.lin([(0, PAL["ink"], .3), (1, PAL["ink"], 0)], C2[0], C2[1], C2[0] + u[0] * 200, C2[1] + u[1] * 200, user=True),
           sw=.5, fill="none", da="2 2")
    um = [um_pts[0], A, um_pts[1]]
    a.path(P(um, True), fill=PAL["slated"], fo=.72)
    a.path(P(um), cls="h", so=.7)
    a.circle(T, 2.2, fill=PAL["ink"])

    # ---- the Moon at new phase: lit on the sunward half, dark towards the Earth
    mg = a.lin([(0, "#e8e6e0"), (.47, PAL["rock1"]), (.53, PAL["slate2"]), (1, PAL["slated"])],
               C2[0] - R2 * u[0], C2[1] - R2 * u[1], C2[0] + R2 * u[0], C2[1] + R2 * u[1], user=True)
    a.circle(C2, R2, fill=mg)
    a.circle(C2, R2, cls="o", sw=.8)
    return a.render()


def _meteorites_rock(a, c, s, seed):
    """A fused-crust meteorite fragment of size s at screen point c: soft shadow to the lower
    right, dark irregular body, a light rim catching the top-left light."""
    rng = __import__("random").Random(seed)
    a.ellipse((c[0] + s * .55, c[1] + s * .18), s * 1.35, s * .42, fill=PAL["ink"], fo=.2)
    k = 11
    pts = []
    for i in range(k):
        t = 2 * math.pi * i / k + math.pi
        r = s * (1 + rng.uniform(-.17, .13))
        pts.append((c[0] + r * 1.2 * math.cos(t), c[1] - s * .3 + r * .72 * math.sin(t)))
    a.path(smooth(pts, close=True), fill=PAL["char"])
    hi = pts[0:4]
    a.path(smooth(hi), stroke=PAL["rock1"], sw=max(.35, s * .16), so=.75, fill="none", lc="round")


def meteorites():
    """A meteoroid's fall: the fireball and ablation trail through the atmosphere, break-up at
    altitude, the fragments' dark flight, and the strewn field on a desert plain where the
    largest pieces carry farthest downrange; the cut face shows a fragment buried in the sand."""
    from .kit import box_faces, ob
    a = Art("amber", seed=111)
    rng = a.rng
    o = (16, 154)
    Wd, D = 232.0, 80.0
    T = 18.0                                    # slab thickness
    yc = 38.0                                   # depth of the ground track (vertical flight plane)
    # air thickening towards the ground (faint sky wash; fades out at the top of the card)
    a.path(P([(0, 30), (320, 30), (320, 180), (0, 180)], True),
           fill=a.lin([(0, PAL["sky2"], 0), (1, PAL["sky2"], .85)], 0, 0, 0, 1))
    # ---- ground slab: a desert plain, cut to show the sand over bedrock
    f, t, r = box_faces(0, Wd, -T, 0, D, o)
    a.path(P(t, True), fill=a.lin([(0, PAL["sand1"]), (1, mix(PAL["sand1"], PAL["sand2"], .55))],
                                  t[1][0], t[1][1], t[0][0], t[0][1], user=True))
    # strata on the cut faces: wind-blown sand, compacted soil, caliche, bedrock
    strata = [(0, 5, PAL["sand2"]), (5, 10, PAL["soil"]), (10, 13, mix(PAL["soil"], PAL["sand1"], .5)),
              (13, T, PAL["clay"])]
    for z0, z1, col in strata:
        a.path(P([ob(0, 0, -z0, o), ob(Wd, 0, -z0, o), ob(Wd, 0, -z1, o), ob(0, 0, -z1, o)], True), fill=col)
        a.path(P([ob(Wd, 0, -z0, o), ob(Wd, D, -z0, o), ob(Wd, D, -z1, o), ob(Wd, 0, -z1, o)], True),
               fill=mix(col, PAL["umber"], .45))
    for z0, _, _ in strata[1:]:
        wav = [ob(x, 0, -z0 + .6 * math.sin(x * .09 + z0), o) for x in range(0, int(Wd) + 1, 8)]
        a.path(smooth(wav) + P([ob(Wd, 0, -z0, o), ob(Wd, D, -z0, o)]), cls="h", so=.3)
    # pebbles in the bedrock and soil
    peb = [ob(rng.uniform(4, Wd - 4), 0, -rng.uniform(6, T - 1.5), o) for _ in range(46)]
    a.dots(peb, r=.45, color=PAL["umber"], op=.55)
    # surface texture: dune ripples running across the plain, and scattered gravel
    cp = a.clip(P(t, True))
    a.group(cp=cp)
    for j in range(11):
        y = 4 + j * 7.2 + rng.uniform(-1, 1)
        ph = rng.uniform(0, 6)
        x = rng.uniform(-10, 10)
        while x < Wd:
            ln = rng.uniform(18, 44)
            rip = [ob(xx, y + 1.4 * math.sin(xx * .06 + ph), 0, o) for xx in [x + ln * i / 6 for i in range(7)]]
            a.path(smooth(rip), stroke=PAL["umber"], sw=.45, so=.26, fill="none", lc="round")
            a.path(smooth([(p[0] - .4, p[1] - .7) for p in rip]), stroke="#fff", sw=.45, so=.5, fill="none", lc="round")
            x += ln + rng.uniform(5, 16)
    grav = [ob(rng.uniform(0, Wd), rng.uniform(0, D), 0, o) for _ in range(70)]
    a.dots(grav, r=.42, color=PAL["rockd"], op=.45)
    # a few dry shrubs out on the plain
    for (sx, sy) in ((24, 56), (58, 20), (42, 66), (118, 64), (214, 66), (198, 8)):
        p = ob(sx, sy, 0, o)
        a.ellipse((p[0] + 1.6, p[1] + .3), 3, .9, fill=PAL["ink"], fo=.15)
        a.path(f"M{n(p[0] - 2.4)} {n(p[1])}q.2 -2.6 2.4 -2.8q2.2.2 2.4 2.8Z", fill=PAL["moss"])
        a.path(f"M{n(p[0] - 1.6)} {n(p[1] - 1.4)}q.6 -1 1.4 -1.1", stroke=PAL["sage1"], sw=.4, so=.8, fill="none", lc="round")
    a.end()
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o")
    # ---- break-up point, high above the uprange end of the strewn field
    bx, bz = 96.0, 92.0
    burst = ob(bx, yc, bz, o)
    # ground track: the flight's projection on the plain, through the nadir of the break-up
    a.path(P([ob(bx - 30, yc, 0, o), ob(232, yc, 0, o)]), stroke=PAL["ink2"], sw=.5, so=.55, da="1.2 2", fill="none")
    nad = ob(bx, yc, 0, o)
    a.path(P([burst, nad]), stroke=PAL["ink3"], sw=.45, so=.6, da="1 2", fill="none")
    a.path(f"M{n(nad[0] - 3)} {n(nad[1])}h6M{n(nad[0])} {n(nad[1] - 1.6)}v3.2", stroke=PAL["ink2"], sw=.6, fill="none")
    # strewn field: an ellipse elongated along the ground track, all downrange of the nadir
    cx, rx, ry = 162.0, 62.0, 19.0
    ell = [ob(cx + rx * math.cos(tt), yc + ry * math.sin(tt), 0, o) for tt in [2 * math.pi * k / 60 for k in range(60)]]
    a.path(P(ell, True), fill=a.AP, fo=.75, stroke=a.AD, sw=.8, da="3 2")
    # fragments, sized by mass: the small ones stop first (uprange), the largest carry farthest
    frags = []
    for k in range(15):
        u = k / 14
        x = cx - rx * .86 + u * rx * 1.74 + rng.uniform(-2.5, 2.5)
        half = ry * .8 * math.sqrt(max(0.0, 1 - ((x - cx) / rx) ** 2))
        y = yc + rng.uniform(-half, half)
        s = .8 + 3.0 * u ** 1.6
        frags.append((x, y, s))
    tracked = [frags[i] for i in (2, 5, 9, 12, 14)]
    # dark flight: ballistic arcs in 3D, horizontal speed bleeding off until the fall is vertical
    for (x, y, s) in tracked:
        pts = []
        for k in range(13):
            u = k / 12
            pts.append(ob(bx + (x - bx) * (1 - (1 - u) ** 3), lerp(yc, y, u), bz * (1 - u ** 1.15), o))
        a.path(smooth(pts), stroke=PAL["ink2"], sw=.7, da="2.6 1.8", fill="none")
    for i, (x, y, s) in enumerate(sorted(frags, key=lambda q: -q[1])):
        p = ob(x, y, 0, o)
        if s > 1.9:   # a shallow pit kicked up by the heavier pieces
            a.ellipse((p[0] + .3, p[1] + .2), s * 1.7, s * .55, fill=PAL["sand2"], stroke=PAL["umber"], sw=.35, so=.5)
        _meteorites_rock(a, p, s, 900 + i)
    # ---- a fragment that buried itself: cross-section in the cut face, with its entry funnel
    fx = 214.0
    top = ob(fx, 0, 0, o)
    tx, ty = top
    a.path(f"M{n(tx - 7)} {n(ty)}q7 3 14 0", fill=mix(PAL["sand2"], PAL["umber"], .3), stroke=PAL["ink"], sw=.45, so=.6)
    a.path(P([(tx - 5, ty + .8), (tx - 2.2, ty + 9.5), (tx + 2.4, ty + 9.5), (tx + 5, ty + .8)], True),
           fill=mix(PAL["sand2"], PAL["sand1"], .55), stroke=PAL["umber"], sw=.4, so=.8, da="1.2 .9")
    by = ty + 11.4
    a.path(f"M{n(tx - 4.2)} {n(by + 1)}q-.4 -3.6 2.6 -4.2q3.8 -.6 4.8 2.2q.6 2.8-2.2 3.4q-3.8.6-5.2 -1.4Z",
           fill=PAL["char"], stroke=PAL["ink"], sw=.4)
    a.path(f"M{n(tx - 3)} {n(by - 1.4)}q1.4 -1.6 3.6 -1.2", stroke=PAL["rock1"], sw=.6, so=.85, fill="none", lc="round")
    # ---- luminous flight: the ablation trail widening and brightening towards the fireball,
    # leaving a faint persistent train of smoke behind it
    entry = (-4, 8)
    u_ = (burst[0] - entry[0], burst[1] - entry[1])
    L = math.hypot(*u_)
    ux, uy = u_[0] / L, u_[1] / L
    px, py = -uy, ux
    train = []
    for k in range(8):
        q = .1 + k * .09
        c = lerp2(entry, burst, q)
        wob = 1.4 * math.sin(k * 1.9)
        train.append((c[0] - px * (2.6 + wob), c[1] - py * (2.6 + wob)))
    a.path(smooth(train), stroke=PAL["cloud2"], sw=5, so=.55, fill="none", lc="round", filter=a.blur(1.4))
    w = 4.2
    a.path(P([entry, (burst[0] + px * w, burst[1] + py * w), (burst[0] - px * w, burst[1] - py * w)], True),
           fill=a.lin([(0, a.AL, 0), (.6, a.AL, .55), (1, a.AL, .95)], entry[0], entry[1], burst[0], burst[1], user=True))
    mid = lerp2(entry, burst, .35)
    a.path(P([mid, burst]), stroke=a.lin([(0, a.A, 0), (1, a.A, 1)], mid[0], mid[1], burst[0], burst[1], user=True),
           sw=2.2, lc="round")
    a.circle(burst, 11, fill=a.rad([(0, "#fffbea"), (.45, a.AL, .85), (1, a.AL, 0)]))
    # sparks of the break-up
    for ang, d in ((-.5, 9), (.15, 11), (.7, 8.5), (1.25, 10)):
        q = _rot((ux, uy), ang)
        a.path(P([(burst[0] + q[0] * 5, burst[1] + q[1] * 5), (burst[0] + q[0] * d, burst[1] + q[1] * d)]),
               stroke=a.A, sw=.7, lc="round", so=.8)
    a.circle(burst, 3.4, fill="#fffdf6", stroke=a.A, sw=1.1)
    return a.render()


def _night_lights_sky(E, v, k):
    """A point at k Earth radii along unit vector v: (screen point, hidden behind the globe)."""
    lon = math.degrees(math.atan2(v[1], v[0]))
    lat = math.degrees(math.asin(max(-1, min(1, v[2]))))
    x, y, cosc = E.raw(lon, lat)
    p = (E.c[0] + k * E.R * x, E.c[1] - k * E.R * y)
    return p, cosc < 0 and k * math.hypot(x, y) < 1


def _night_lights_land(E, simplify=1.6, min_area=5.0):
    """Coarse land for card size: only rings mostly on the visible hemisphere, thinned hard, and
    without islands or limb slivers smaller than about 2 px across at card size."""
    from .kit import countries, _thin
    out = []
    for _, ring in countries():
        pts, vis = [], 0
        for lon, lat in ring:
            p, v = E(lon, lat)
            vis += v
            pts.append(p)
        if not vis:
            continue
        pts = _thin(pts, simplify)
        k = range(len(pts))
        area = abs(sum(pts[i][0] * pts[i - 1][1] - pts[i - 1][0] * pts[i][1] for i in k)) / 2
        perim = sum(math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]) for i in k)
        # drop tiny islands, and slivers (partly hidden rings squashed along the limb: area/perimeter tiny)
        if len(pts) >= 4 and area >= min_area and (vis == len(ring) or area > .8 * perim):
            out.append(P(pts, True))
    return "".join(out)


def _night_lights_shade(a, E, disc, day):
    """Sphere shading lit from the top left; the white highlight is kept on the day side only."""
    hi = a.rad([(0, "#ffffff", .35), (.45, "#ffffff", 0)], .36, .3, .78)
    a.path(day, fill=hi)
    dk = a.rad([(0, PAL["ink"], 0), (.45, PAL["ink"], 0), (.8, PAL["ink"], .12), (1, PAL["ink"], .3)], .36, .3, .78)
    a.path(disc, fill=dk)
    a.path(disc, cls="o")


def _night_lights_hull(pts):
    """Convex hull of screen points (monotone chain), counter-clockwise."""
    pts = sorted(set((round(x, 2), round(y, 2)) for x, y in pts))
    def half(seq):
        h = []
        for p in seq:
            while len(h) >= 2 and ((h[-1][0] - h[-2][0]) * (p[1] - h[-2][1]) - (h[-1][1] - h[-2][1]) * (p[0] - h[-2][0])) <= 0:
                h.pop()
            h.append(p)
        return h
    lo, up = half(pts), half(reversed(pts))
    return lo[:-1] + up[:-1]


def _night_lights_box(E, lon0, lon1, lat0, lat1, k=8):
    """Outline of a lon/lat box on the globe, edges sampled so they follow the sphere."""
    ring = ([(lon0 + (lon1 - lon0) * i / k, lat0) for i in range(k)] +
            [(lon1, lat0 + (lat1 - lat0) * i / k) for i in range(k)] +
            [(lon1 - (lon1 - lon0) * i / k, lat1) for i in range(k)] +
            [(lon0, lat1 - (lat1 - lat0) * i / k) for i in range(k)])
    return [E(lon, lat)[0] for lon, lat in ring]


def night_lights():
    """The Earth across the day-night line: cities glow along the coasts and river plains of the
    night side, and an Earth-observation satellite in orbit above it images a scene of those
    lights through its sensor's viewing cone."""
    from .kit import ang_dist
    a = Art("amber", seed=113)
    E = Ortho(70, 26, 72, (124, 94))
    disc, cp = globe(a, E, land=False, grat=False)
    a.group(cp=cp)
    land = a.uid("land")
    a.defs.append(f'<path id="{land}" d="{_night_lights_land(E)}"/>')
    a.add(f'<use href="#{land}" fill="{PAL["sand2"]}" stroke="{PAL["sand2"]}" stroke-width="1.1" stroke-linejoin="round"/>')
    sub = (-30, 14)                    # subsolar point: the Sun off to the upper left, matching the shading
    anti = (sub[0] + 180, -sub[1])
    night = E.cap(anti, 90)
    a.path(night, fill="#1d2532", fo=.92)
    a.add(f'<use href="#{land}" fill="#39414f" stroke="#39414f" stroke-width="1.1" stroke-linejoin="round" clip-path="{a.clip(night)}"/>')
    # the scene the satellite images: a lon/lat box over eastern China
    box = (102, 126, 20, 42)
    foot = _night_lights_box(E, *box)
    a.path(P(foot, True), fill=a.AL, fo=.13)
    # city lights: every sizeable night-side city, brighter inside the imaged scene
    lights = []
    for (lon, lat, pop, _) in sorted(cities(), key=lambda c: -c[2]):
        if pop < 6e5 or ang_dist((lon, lat), anti) > 86:
            continue
        p, v = E(lon, lat, clamp=False)
        if not v or math.hypot(p[0] - E.c[0], p[1] - E.c[1]) > E.R - 2.5:
            continue
        if any(math.hypot(p[0] - q[0], p[1] - q[1]) < 3.2 for q, _, _ in lights):
            continue
        seen = box[0] <= lon <= box[1] and box[2] <= lat <= box[3]
        lights.append((p, pop, seen))
    for p, pop, seen in lights:
        s = min(1, pop / 2e7) ** .5
        a.circle(p, 1.8 + 3.2 * s, fill=a.AL, fo=.34 if seen else .2)
    for p, pop, seen in lights:
        s = min(1, pop / 2e7) ** .5
        a.circle(p, .7 + .9 * s, fill="#fff3d6" if seen else "#f6d9a0", fo=1 if seen else .85)
    a.end()
    _night_lights_shade(a, E, disc, E.cap(sub, 90))
    a.path(P(foot, True), fill="none", stroke=a.AL, sw=.8, so=.9, lj="round")
    # the satellite in orbit above the night side (size and height exaggerated), its orbit a dashed arc
    sat = (252, 38)
    arc = [(36, 70), (76, 32), (140, 14), (204, 18), sat, (288, 62), (306, 88)]
    a.path(smooth(arc), cls="h", da="2 2", so=.55)
    g, rot = 2.3, -10                  # satellite drawn larger than true scale, so it reads on the card
    r = math.radians(rot)
    eye = (sat[0] - 4 * g * math.sin(r), sat[1] + 4 * g * math.cos(r))   # the sensor under the body
    eye = (round(eye[0], 2), round(eye[1], 2))
    cone = _night_lights_hull([eye] + foot)
    a.path(P(cone, True), fill=a.lin([(0, a.AL, .45), (1, a.AL, .12)], *eye, *E(114, 31)[0], user=True))
    i = cone.index(eye)
    for c in (cone[i - 1], cone[(i + 1) % len(cone)]):   # the cone's two silhouette edges
        a.path(P([eye, c]), stroke=a.A, sw=.6, so=.7)
    x, y = sat
    a.add(f'<g transform="translate({n(x)} {n(y)}) rotate({rot}) scale({g})">')
    a.path("M-12 -2.6h7.6v5.2h-7.6ZM4.4 -2.6h7.6v5.2h-7.6Z", fill=PAL["slate2"], stroke=PAL["ink"], sw=.45)
    a.path("M-9.5 -2.6v5.2M-7 -2.6v5.2M6.9 -2.6v5.2M9.4 -2.6v5.2", stroke="#ffffff", sw=.3, so=.5)
    a.path("M-5 0h10", stroke=PAL["ink"], sw=.6)
    a.path("M-2.8 -3.4h5.6v6.4h-5.6Z", fill="#e8e4da", stroke=PAL["ink"], sw=.55)
    a.path("M-1.4 3h2.8l-.6 1.4h-1.6Z", fill=PAL["ink"])
    a.add("</g>")
    return a.render()
