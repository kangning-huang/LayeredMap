"""Transport: airports, ports, railways."""
import csv
import math

from .kit import (PAL, REPO, Art, P, smooth, ob, mix, n, Ortho, globe, globe_shade, great_circle, lerp2, ring_pts)

_AIRPORTS = None


def airports_csv():
    global _AIRPORTS
    if _AIRPORTS is None:
        with open(REPO / "assets" / "examples" / "airports.csv", encoding="utf-8") as f:
            _AIRPORTS = {r["iata"] or r["name"]: r for r in csv.DictReader(f)}
    return _AIRPORTS


def _ll(code):
    r = airports_csv()[code]
    return float(r["longitude"]), float(r["latitude"])


def _runway(length, width, thr_bars=6):
    """A runway along the local x axis, centred on the origin, with ICAO-style markings:
    threshold piano keys, aiming-point blocks, touchdown-zone bars and a dashed centreline."""
    L, Wd = length / 2, width / 2
    s = [f'<rect x="{n(-L)}" y="{n(-Wd)}" width="{n(length)}" height="{n(width)}" fill="{PAL["slate2"]}"/>']
    marks = []
    bw = width / (thr_bars * 2 + 1)
    for end in (-1, 1):
        x0 = end * (L - 0.8)
        for i in range(thr_bars):                       # piano keys
            y = -Wd + bw * (2 * i + 1)
            marks.append(f"M{n(x0)} {n(y + bw / 2)}h{n(-end * 4.2)}")
        xa = end * (L - length * 0.17)                  # aiming points
        marks.append(f"M{n(xa)} {n(-Wd * .55)}h{n(-end * 3.4)}M{n(xa)} {n(Wd * .55)}h{n(-end * 3.4)}")
        for k in (1, 2):                                # touchdown-zone bars
            xt = end * (L - length * (0.1 + 0.11 * k))
            marks.append(f"M{n(xt)} {n(-Wd * .6)}h{n(-end * 1.6)}M{n(xt)} {n(Wd * .6)}h{n(-end * 1.6)}")
    s.append(f'<path d="{"".join(marks)}" stroke="#fff" stroke-width="{n(bw * 0.9) if bw > .3 else ".3"}" stroke-opacity=".9" fill="none"/>')
    s.append(f'<path d="M{n(-L + 8)} 0H{n(L - 8)}" stroke="#fff" stroke-width=".35" stroke-dasharray="2.4 1.8" stroke-opacity=".9"/>')
    s.append(f'<rect x="{n(-L)}" y="{n(-Wd)}" width="{n(length)}" height="{n(width)}" fill="none" stroke="{PAL["ink"]}" stroke-width=".4"/>')
    # approach lighting beyond each threshold: a centre row with a crossbar
    s.append(f'<path d="M{n(L + 1)} 0h9M{n(L + 6)} {n(-Wd * .9)}v{n(width * .9)}M{n(-L - 1)} 0h-9M{n(-L - 6)} {n(-Wd * .9)}v{n(width * .9)}" '
             f'stroke="{PAL["ink2"]}" stroke-width=".5" stroke-dasharray=".8 .7" fill="none"/>')
    return "".join(s)


def _plane(x, y, ang, s=1.0, fill="#fff"):
    """A tiny airliner silhouette (nose along +x)."""
    d = ("M5 0L3.6 .5H1L-1.6 3.6H-2.6L-1.4 .5H-3.8L-4.8 1.6H-5.4L-4.8 0L-5.4-1.6H-4.8L-3.8-.5H-1.4"
         "L-2.6-3.6H-1.6L1-.5H3.6Z")
    return (f'<path d="{d}" transform="translate({n(x)} {n(y)}) rotate({n(ang)}) scale({s})" fill="{fill}" '
            f'stroke="{PAL["ink"]}" stroke-width="{.3 / s:.2f}"/>')


def _airports_runway(length, width):
    """A simplified runway along local x: asphalt, broad threshold bars and a dashed centreline."""
    L, Wd = length / 2, width / 2
    s = f'<rect x="{n(-L)}" y="{n(-Wd)}" width="{n(length)}" height="{n(width)}" fill="{PAL["slate2"]}" stroke="{PAL["ink"]}" stroke-width=".45"/>'
    bars = ""
    for end in (-1, 1):
        x0 = end * (L - 1)
        for y in (-Wd * .55, 0, Wd * .55):
            bars += f"M{n(x0)} {n(y)}h{n(-end * 5)}"
    s += f'<path d="{bars}" stroke="#fff" stroke-width="1.1" fill="none"/>'
    s += f'<path d="M{n(-L + 9)} 0H{n(L - 9)}" stroke="#fff" stroke-width=".8" stroke-dasharray="4 3" fill="none"/>'
    return s


def airports():
    """A magnified airport plan (two parallel runways, taxiways and a terminal) tied to its place on
    a globe, where great-circle routes from New York bow poleward; the route to Madrid is set against
    the parallel both cities share, the longer constant-heading track."""
    a = Art("cobalt", seed=7)
    # centred south of the JFK-MAD route so its great circle bows visibly poleward on screen
    G = Ortho(-38, 24, 74, (240, 94))
    a.shadow(250, 171, 62, 4, op=.16)
    disc, cp = globe(a, G, land=False, grat=False)
    a.group(cp=cp)
    # land and graticule at a coarser tolerance than the kit default (lighter SVG, same read at card size)
    a.path(G.land(simplify=.9), fill=PAL["sand1"], stroke=PAL["sand1"], sw=.6, lj="round")
    a.path(G.graticule(step=30, res=15), cls="h", so=.16)
    # large airports in the bundled layer, thinned so no two dots sit closer than 5 px
    pts = []
    for r in airports_csv().values():
        if r["type"] != "large":
            continue
        p, v = G(float(r["longitude"]), float(r["latitude"]), clamp=False)
        if v:
            pts.append((round(p[0], 1), round(p[1], 1)))
    kept = []
    for p in sorted(set(pts)):
        if all((p[0] - k[0]) ** 2 + (p[1] - k[1]) ** 2 > 5 ** 2 for k in kept):
            kept.append(p)
    a.dots(kept, r=.7, color=PAL["ink2"], op=.45)
    # the shared parallel (constant heading) between JFK and MAD, dashed, for comparison
    (lon0, lat0), (lon1, lat1) = _ll("JFK"), _ll("MAD")
    lat = (lat0 + lat1) / 2
    a.path(G.line([(lon0 + (lon1 - lon0) * i / 24, lat) for i in range(25)], curve=True),
           fill="none", stroke=PAL["ink"], sw=.8, so=.6, da="2.2 1.8", lc="round")
    dests = ("LHR", "MAD", "GRU")
    for q in dests:
        d = G.line(great_circle(_ll("JFK"), _ll(q), 24), curve=True)
        a.path(d, fill="none", stroke="#fff", sw=2.4, so=.75, lc="round")
        a.path(d, fill="none", stroke=a.A, sw=1.25, lc="round")
    for q in dests:
        p, v = G(*_ll(q), clamp=False)
        if v:
            a.circle(p, 1.7, fill="#fff", stroke=a.AD, sw=.7)
    a.end()
    globe_shade(a, G, disc, op=.2)
    jfk, _ = G(*_ll("JFK"))
    a.circle(jfk, 4.2, fill="none", stroke=PAL["ink"], sw=.9)
    a.circle(jfk, 1.7, fill=a.A, stroke="#fff", sw=.6)

    # ---- magnified plan view, linked to JFK by tangent leader lines
    C, R = (78, 92), 62
    dx, dy = jfk[0] - C[0], jfk[1] - C[1]
    dist = math.hypot(dx, dy)
    base = math.atan2(dy, dx)
    off = math.acos((R - 4.2) / dist)
    for s in (1, -1):
        p = (C[0] + R * math.cos(base + s * off), C[1] + R * math.sin(base + s * off))
        q = (jfk[0] + 4.2 * math.cos(base + s * off), jfk[1] + 4.2 * math.sin(base + s * off))
        a.path(P([p, q]), cls="h", so=.6)
    lens = f"M{n(C[0] - R)} {n(C[1])}a{n(R)} {n(R)} 0 1 0 {n(2 * R)} 0a{n(R)} {n(R)} 0 1 0 {n(-2 * R)} 0Z"
    a.shadow(C[0] + 6, C[1] + R + 2, R * .9, 5, op=.18)
    lcp = a.clip(lens)
    a.path(lens, fill=a.lin([(0, "#eef1e4"), (1, PAL["sage2"])], 0, 0, 1, 1))
    a.group(cp=lcp)
    tw = PAL["rock1"]
    g = f'<g transform="translate({n(C[0])} {n(C[1])}) rotate(-38)">'
    # one taxiway joining both runway ends past the apron, then the apron itself
    # end taxiways join the runway ends; a stub from each long side of the apron reaches its runway
    g += (f'<path d="M-44-26V26M44-26V26M24-26V-15M-24 26V15" stroke="{tw}" stroke-width="3.6" fill="none"/>'
          f'<rect x="-34" y="-15" width="68" height="30" rx="2" fill="{mix(PAL["rock1"], PAL["paper"], .35)}" stroke="{PAL["ink"]}" stroke-width=".45"/>')
    g += f'<g transform="translate(0 -26)">{_airports_runway(100, 9)}</g>'
    g += f'<g transform="translate(0 26)">{_airports_runway(100, 9)}</g>'
    # terminal: a spine with two piers, an airliner nosed in at each
    g += (f'<path d="M-28-2H28V2H-28ZM-10-2V-12H-6V-2ZM6 2V12H10V2Z" fill="{PAL["paper"]}" stroke="{PAL["ink"]}" '
          f'stroke-width=".55" stroke-linejoin="round"/>')
    g += _plane(-18.6, -8.2, 0, 1.7) + _plane(18.6, 8.2, 180, 1.7)
    g += "</g>"
    a.add(g)
    a.end()
    a.path(lens, cls="o")
    return a.render()


def ports():
    """A berth in an oblique block: a container ship moored alongside the quay, a ship-to-shore
    gantry crane on its rails reaching out over the ship, and, in the cut face, the berth pocket
    dredged below the natural seabed (dashed) so that the channel depth exceeds the ship's draught."""
    from .kit import box3, pt
    a = Art("verm", seed=71)
    o = (38, 168)
    W, D = 150.0, 130.0                       # block width (across the quay) and depth (along it)
    QX, QZ, WL = 54.0, 60.0, 52.0             # quay face, quay level, waterline
    BED, NAT = 20.0, 36.0                     # dredged pocket and natural seabed
    DECK, KEEL, SX0, SX1, L = 68.0, 30.0, 62.0, 90.0, 124.0

    def bed(x):
        if x <= 114:
            return BED
        if x >= 136:
            return NAT
        t = (x - 114) / 22
        return BED + (NAT - BED) * (3 * t * t - 2 * t ** 3)

    def F(x, z):                              # a point on the front (cut) face
        return ob(x, 0, z, o)

    a.shadow(o[0] + 125, o[1] + 3, 125, 6, op=.2)
    # ---- block: quay fill at the left, water over the seabed at the right
    xs = [QX + i * 2 for i in range(int((W - QX) / 2) + 1)]
    bedline = [F(x, bed(x)) for x in xs]
    a.path(P([F(0, 0), F(QX, 0), F(QX, QZ), F(0, QZ)], True), fill=a.lin([(0, PAL["rock1"]), (1, PAL["rock"])]))
    a.path(P([F(QX, 0), F(W, 0)] + bedline[::-1], True), fill=a.lin([(0, PAL["sand2"]), (1, PAL["clay"])]))
    a.path(P(bedline + [F(W, WL), F(QX, WL)], True), fill=a.lin([(0, PAL["water3"]), (1, PAL["water4"])]))
    # right face: seabed and water
    a.path(P([ob(W, 0, 0, o), ob(W, D, 0, o), ob(W, D, NAT, o), ob(W, 0, NAT, o)], True), fill=mix(PAL["clay"], PAL["ink"], .15))
    a.path(P([ob(W, 0, NAT, o), ob(W, D, NAT, o), ob(W, D, WL, o), ob(W, 0, WL, o)], True), fill=PAL["water4"])
    # top faces: quay apron and water surface
    a.path(P([ob(0, 0, QZ, o), ob(0, D, QZ, o), ob(QX, D, QZ, o), ob(QX, 0, QZ, o)], True), fill=mix(PAL["rock1"], PAL["paper"], .3))
    a.path(P([ob(QX, 0, WL, o), ob(QX, D, WL, o), ob(W, D, WL, o), ob(W, 0, WL, o)], True), fill=PAL["water2"])
    # berth face of the quay wall, above the water
    a.path(P([ob(QX, 0, QZ, o), ob(QX, D, QZ, o), ob(QX, D, WL, o), ob(QX, 0, WL, o)], True), fill=PAL["rock"])
    # crane rails along the quay
    a.path(P([ob(4, 0, QZ, o), ob(4, D, QZ, o)]) + P([ob(28, 0, QZ, o), ob(28, D, QZ, o)]), cls="h")
    # natural seabed before dredging (dashed) and the seabed line
    a.path(P([F(QX, NAT), F(136, NAT)]), stroke=PAL["ink"], sw=1.3, da="3.5 2", so=.9, fill="none")
    a.path(P(bedline), stroke=PAL["ink"], sw=.8, fill="none", lj="round")
    # block outline
    a.path(P([F(0, QZ), F(0, 0), F(W, 0), ob(W, D, 0, o), ob(W, D, WL, o), ob(QX, D, WL, o), ob(QX, D, QZ, o),
              ob(0, D, QZ, o), F(0, QZ)]) + P([F(0, QZ), F(QX, QZ), F(QX, BED)]) + P([F(QX, WL), F(W, WL), F(W, 0)])
           + P([F(W, WL), ob(W, D, WL, o)]) + P([F(QX, QZ), ob(QX, D, QZ, o)]), cls="o")

    # ---- the ship, stern in the cut face, bow away from us
    stern = (f"M{pt(F(SX0, DECK))}L{pt(F(SX1, DECK))}L{pt(F(SX1, KEEL + 6))}Q{pt(F(SX1, KEEL))} {pt(F(SX1 - 6, KEEL))}"
             f"L{pt(F(SX0 + 6, KEEL))}Q{pt(F(SX0, KEEL))} {pt(F(SX0, KEEL + 6))}Z")
    a.path(stern, fill=PAL["slate2"])
    below = a.clip(P([(0, F(0, WL)[1]), (320, F(0, WL)[1]), (320, 180), (0, 180)], True))
    a.path(stern, fill=a.A, cp=below)
    a.path(P([F(SX0, WL), F(SX1, WL)]), stroke="#fff", sw=.7, so=.8)
    a.path(stern, cls="o")
    # a long, fine bow: with a 30-unit taper the starboard flank faces the viewer (the port one is hidden)
    bowy, mid = L - 30, (SX0 + SX1) / 2
    tip, tipw = ob(mid, L, DECK, o), ob(mid, L, WL, o)
    a.path(P([ob(SX1, 0, DECK, o), ob(SX1, bowy, DECK, o), ob(SX1, bowy, WL, o), ob(SX1, 0, WL, o)], True), fill=PAL["slated"])
    a.path(P([ob(SX1, bowy, DECK, o), tip, tipw, ob(SX1, bowy, WL, o)], True), fill=mix(PAL["slated"], PAL["ink"], .15))
    deck = P([ob(SX0, 0, DECK, o), ob(SX0, bowy, DECK, o), tip, ob(SX1, bowy, DECK, o), ob(SX1, 0, DECK, o)], True)
    a.path(deck, fill=PAL["slate1"])
    a.path(P([ob(SX1, bowy, DECK, o), ob(SX1, bowy, WL, o)]), cls="h")
    a.path(deck + P([ob(SX1, 0, WL, o), ob(SX1, bowy, WL, o), tipw, tip]), cls="o")
    cols = [PAL["sand2"], PAL["slate1"], PAL["sage2"]]

    def bay(y0, y1, tiers, c):
        # containers stowed lengthwise (bow to stern): three rows across the beam, seen end-on in front
        x0, x1, z1 = SX0 + 2, SX1 - 2, DECK + tiers * 6
        box3(a, x0, x1, y0, y1, DECK, z1, o, c, mix(c, "#fff", .35), mix(c, PAL["ink"], .2), sw=.5)
        g = "".join(P([ob(x1, y0, DECK + 6 * t, o), ob(x1, y1, DECK + 6 * t, o)])
                    + P([ob(x0, y0, DECK + 6 * t, o), ob(x1, y0, DECK + 6 * t, o)]) for t in range(1, tiers))
        g += "".join(P([ob(x, y0, DECK, o), ob(x, y0, z1, o), ob(x, y1, z1, o)]) for x in (x0 + 8, x0 + 16))
        a.path(g, cls="h", so=.45)

    bay(70, 84, 3, cols[0])                   # the bays beyond the crane, drawn first
    bay(54, 68, 3, cols[2])
    # ---- ship-to-shore crane: two portal frames on the quay rails, boom out over the ship
    cy, top = 44.0, 122.0
    C = lambda x, z, y=cy: ob(x, y, z, o)

    def frame(y, col, brace):
        d = P([C(4, QZ, y), C(4, top, y)]) + P([C(28, QZ, y), C(28, top, y)]) + P([C(4, 90, y), C(28, 90, y)])
        if brace:
            d += P([C(4, QZ, y), C(28, 90, y)]) + P([C(28, QZ, y), C(4, 90, y)])
        a.path(d, stroke=col, sw=1.8, fill="none", lc="butt")
    frame(cy + 5, mix(PAL["slated"], PAL["paper"], .35), False)
    a.path(P([C(4, top, cy - 5), C(4, top, cy + 5)]) + P([C(28, top, cy - 5), C(28, top, cy + 5)]),
           stroke=PAL["slated"], sw=1.8, fill="none", lc="butt")
    a.path(P([C(0, top), C(106, top)]), stroke=PAL["slated"], sw=2.6, fill="none", lc="butt")
    a.path(P([C(0, top), C(16, top + 16), C(106, top)]) + P([C(4, top), C(16, top + 16), C(28, top)]),
           stroke=PAL["slated"], sw=.9, fill="none")
    frame(cy - 5, PAL["slated"], True)
    tx = mid
    a.path(P([C(tx - 6, top - 1.5), C(tx + 6, top - 1.5)]), stroke=PAL["slate2"], sw=4, fill="none", lc="butt")
    a.path(P([C(tx - 4, top - 3), C(tx - 4, 108)]) + P([C(tx + 4, top - 3), C(tx + 4, 108)]), stroke=PAL["ink"], sw=.6, fill="none")
    box3(a, tx - 11, tx + 11, cy - 4, cy + 4, 102, 108, o, a.AL, mix(a.AL, "#fff", .4), mix(a.AL, PAL["ink"], .2), sw=.5)
    # bays in front of the crane, and the bridge at the stern
    bay(19, 33, 2, cols[1])
    box3(a, SX0 + 4, SX1 - 4, 3, 15, DECK, 98, o, "#fbfaf6", "#fff", mix("#fbfaf6", PAL["ink"], .15), sw=.6)
    a.path(P([ob(SX0 + 4, 3, 93, o), ob(SX1 - 4, 3, 93, o)]), stroke=PAL["slated"], sw=2.2, fill="none", lc="butt")

    # ---- draught (ink) and channel depth (accent), side by side
    a.path(P([F(SX1, KEEL), F(99, KEEL)]), stroke=PAL["ink"], sw=1.0, da="2 1.5", so=.9, fill="none")
    a.arrow([F(96, WL - .2), F(96, KEEL + .2)], color=PAL["ink"], w=1.4, head=4.4, both=True, curve=False)
    a.arrow([F(106, WL - .2), F(106, BED + .4)], color=a.AD, w=1.4, head=4.4, both=True, curve=False)
    return a.render()


def litho_dots(a):
    ink = mix(PAL["sand2"], PAL["ink"], .4)
    return a.pattern(5, 4.4, f'<circle cx="1" cy="1" r=".4" fill="{ink}"/><circle cx="3.5" cy="3.2" r=".4" fill="{ink}"/>')


def _railways_outline(a, o, Wd, D, base, h, nx, ny):
    """The block outline as terrain() draws it, except that the far-edge hairline is only drawn
    where it is not hidden behind nearer ground (here the ridge crest hides part of it)."""
    from .kit import DX
    xs = [Wd * i / nx for i in range(nx + 1)]
    ys = [D * j / ny for j in range(ny + 1)]
    a.path(P([ob(x, 0, h(x, 0), o) for x in xs]), cls="o")
    a.path(P([ob(Wd, y, h(Wd, y), o) for y in ys]), cls="o")
    a.path(P([ob(0, 0, h(0, 0), o), ob(0, 0, -base, o), ob(Wd, 0, -base, o), ob(Wd, D, -base, o), ob(Wd, D, h(Wd, D), o)])
           + P([ob(Wd, 0, -base, o), ob(Wd, 0, h(Wd, 0), o)]), cls="o")

    # back skyline: the upper envelope of the whole surface seen from the viewpoint, so the
    # far edge and any nearer crest that rises above it join into one unbroken line
    env = {}
    for i in range(int(Wd) + 1):
        for j in range(int(D * 4) + 1):
            x, y = float(i), j / 4
            sx, sy = ob(x, y, h(x, y), o)
            k = round(sx / 1.5)
            if k not in env or sy < env[k][1]:
                env[k] = (sx, sy)
    a.path(P([env[k] for k in sorted(env)]), cls="h", so=.6)


def railways():
    """A high-speed line kept level through hilly ground: it runs at grade on the left
    plateau, crosses the river valley on a viaduct and bores through the ridge in a tunnel,
    whose section shows in the right cut face (the hidden alignment is dashed)."""
    from .kit import terrain, conifer, box3, DX
    a = Art("verm", seed=73)
    o = (14, 148)
    Wd, D, base = 200.0, 132.0, 27.0
    Lz, ty = 22.0, 44.0          # rail level and depth of the line
    knots = [(0, Lz), (32, Lz), (62, 5), (108, 4), (134, 18), (188, 56), (200, 56)]

    def prof(x):
        for (x0, z0), (x1, z1) in zip(knots, knots[1:]):
            if x <= x1:
                t = (x - x0) / (x1 - x0)
                return z0 + (z1 - z0) * t * t * (3 - 2 * t)
        return knots[-1][1]

    def rx(y):                    # river centreline, crossing under the line
        return 86

    def h(x, y):
        u = min(1.0, y / ty)                           # ground falls gently toward the front
        z = prof(x) * (.6 + .4 * u * u * (3 - 2 * u)) - 2.0 * math.exp(-((x - rx(y)) / 8) ** 2)
        return z

    def cover(x, y):
        return "forest" if h(x, y) > 30 else "field"
    cols = {"forest": PAL["forest"], "field": mix(PAL["sage1"], PAL["sage2"], .55)}
    a.shadow(o[0] + 170, o[1] + base + 2, 150, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=30, ny=10, cover=cover, colours=cols, base=base, levels=5, outline=False,
                   strata=[(4, PAL["umber"]), (12, PAL["clay"]), (99, PAL["rock"])])
    # the river along the valley floor, under the viaduct
    ys = [D * j / 12 for j in range(13)]
    bank = [surf(rx(y) - 4, y, .2) for y in ys] + [surf(rx(y) + 4, y, .2) for y in reversed(ys)]
    a.path(P(bank, True), fill=PAL["water3"], stroke=PAL["water4"], sw=.5, lj="round")
    _railways_outline(a, o, Wd, D, base, h, 30, 10)
    # a handful of conifers on the ridge, back to front, clear of the tunnel line
    trees = [(188, 124), (170, 114), (192, 100), (176, 88), (184, 30), (192, 16)]
    a.path("".join(conifer(*surf(x, y, -.3), 13, 6) for x, y in trees),
           fill=PAL["forestd"], stroke=mix(PAL["forestd"], PAL["ink"], .35), sw=.35, lj="round")
    # where the ground leaves and regains rail level
    def cross(x0, x1):
        for i in range(400):
            x = x0 + (x1 - x0) * i / 400
            if (prof(x) - Lz) * (prof(x0) - Lz) <= 0:
                return x
        return x1
    xv0, xp = 32.0, cross(108, 190)                  # viaduct starts where the plateau ends; tunnel portal
    r = 6.5
    zc = Lz + 1.8                                     # springing line of the bore

    def arch(x, rr, z0):                              # tunnel arch in the plane x = const
        return ([ob(x, ty - rr, z0, o)] + [ob(x, ty - rr * math.cos(t * math.pi / 12), zc + rr * math.sin(t * math.pi / 12), o)
                for t in range(13)] + [ob(x, ty + rr, z0, o)])
    # hidden alignment through the ridge (dashed), all the way from the portal to the bore in the cut face
    a.path(P([ob(xp, ty, Lz + 2, o), ob(Wd, ty, Lz + 2, o)]), stroke=a.A, sw=1.5, da="3 2.2", fill="none", so=.9)
    # tunnel bore in section on the right cut face (plane x = Wd)
    a.path(P(arch(Wd, r, Lz - 1), True), fill=PAL["slated"], stroke=PAL["rock1"], sw=1.4, lj="round")
    a.path(P(arch(Wd, r, Lz - 1), True), fill="none", stroke=PAL["ink"], sw=.5)
    a.path(P([ob(Wd, ty - 3.4, Lz - .2, o), ob(Wd, ty + 3.4, Lz - .2, o)]), stroke=a.A, sw=1.8)
    # track at grade on the plateau
    a.path(P([ob(0, ty, Lz, o), ob(xv0, ty, Lz, o)]), stroke=PAL["rock"], sw=5, lc="butt")
    a.path(P([ob(0, ty, Lz, o), ob(xv0, ty, Lz, o)]), stroke=PAL["char"], sw=1, lc="butt")
    # portal: a stone ring with the dark mouth, set into the foot of the ridge where the line meets it
    a.path(P(arch(xp, r + 1.6, Lz - 3.2), True), fill=PAL["rock1"], stroke=PAL["ink"], sw=.5, so=.75, lj="round")
    a.path(P(arch(xp, r, Lz - .2), True), fill=PAL["slated"], stroke=PAL["ink"], sw=.4, lj="round")
    # viaduct: piers standing on the valley floor (with contact shadows), then the deck on top of them
    for x in range(int(xv0) + 12, int(xp) - 4, 13):
        g = min(h(x - 2, ty - 2.2), h(x + 2, ty - 2.2)) - .3     # foot on the ground in front
        if Lz - 3.2 - g > 2:
            a.path(P([ob(x - 3.2, ty - 3.4, g + .3, o), ob(x + 4.5, ty - 3.4, g + .3, o), ob(x + 4.5, ty + 3, g + .3, o),
                      ob(x - 3.2, ty + 3, g + .3, o)], True), fill=PAL["ink"], fo=.22)
            box3(a, x - 2, x + 2, ty - 2.2, ty + 2.2, g, Lz - 3.2, o, PAL["rock1"], None, PAL["rock"], sw=.4)
    # abutment where the deck leaves the plateau
    box3(a, xv0, xv0 + 3, ty - 3.4, ty + 3.4, min(h(xv0 + 3, ty - 3.4), h(xv0, ty - 3.4)) - .4, Lz - 3.2, o,
         PAL["rock1"], None, PAL["rock"], sw=.45)
    box3(a, xv0, xp, ty - 3, ty + 3, Lz - 3.2, Lz, o, PAL["rock1"], mix(PAL["rock1"], "#fff", .35), PAL["rock"], sw=.45)
    a.path(P([ob(xv0, ty, Lz, o), ob(xp, ty, Lz, o)]), stroke=PAL["char"], sw=1, lc="butt")
    # the train on the viaduct, heading for the tunnel
    t0, t1 = 30.0, 86.0
    zb, zt, hw = Lz + .4, Lz + 6.2, 2.4
    box3(a, t0, t1, ty - hw, ty + hw, zb, zt, o, "#fbfaf6", "#ffffff", PAL["cloud2"], sw=.55)
    a.path(P([ob(t1, ty - hw, zt, o), ob(t1, ty + hw, zt, o), ob(t1 + 11, ty + hw, zb + .8, o), ob(t1 + 11, ty - hw, zb + .8, o)], True),
           fill=PAL["cloud2"], stroke=PAL["ink"], sw=.55, lj="round")
    a.path(P([ob(t1, ty - hw, zt, o), ob(t1 + 11, ty - hw, zb + .8, o), ob(t1 + 11, ty - hw, zb, o), ob(t1, ty - hw, zb, o)], True),
           fill="#fbfaf6", stroke=PAL["ink"], sw=.55, lj="round")
    a.path(P([ob(t0, ty - hw, zb + 1.5, o), ob(t1 + 8.5, ty - hw, zb + 1.5, o)]), stroke=a.A, sw=1.8, lc="butt")
    a.path(P([ob(t0 + 2, ty - hw, zb + 4, o), ob(t1 - 1, ty - hw, zb + 4, o)]), stroke=PAL["slate2"], sw=1.5, lc="butt")
    return a.render()
