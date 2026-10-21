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


def airports():
    """A magnified airport plan (parallel and crosswind runways with standard markings, taxiways,
    a pier terminal with aircraft at the gates) tied to its location on a globe, where
    great-circle routes bow poleward across the North Atlantic between real hub airports."""
    a = Art("cobalt", seed=7)
    G = Ortho(-38, 40, 74, (240, 96))
    disc, cp = globe(a, G, land=PAL["sand1"], border=True)
    a.group(cp=cp)
    # every large airport in the bundled layer, as faint dots
    dots = []
    for r in airports_csv().values():
        if r["type"] != "large":
            continue
        p, v = G(float(r["longitude"]), float(r["latitude"]), clamp=False)
        if v:
            dots.append(p)
    a.dots(sorted(set((round(x), round(y)) for x, y in dots)), r=.55, color=PAL["ink2"], op=.5)
    routes = [("JFK", "LHR"), ("JFK", "CDG"), ("JFK", "FRA"), ("JFK", "MAD"), ("BOS", "AMS"), ("ORD", "LHR"),
              ("ATL", "CDG"), ("YYZ", "FRA"), ("MIA", "MAD"), ("JFK", "IST"), ("DFW", "LHR"), ("GRU", "MAD"),
              ("JFK", "GRU"), ("KEF", "JFK"), ("LOS", "LHR")]
    hubs = set()
    for i, (p, q) in enumerate(routes):
        d = G.line(great_circle(_ll(p), _ll(q), 16), curve=True)
        if "JFK" in (p, q):
            a.path(d, fill="none", stroke="#fff", sw=1.9, so=.7, lc="round")
        a.path(d, fill="none", stroke=a.A, sw=.95 if p == "JFK" or q == "JFK" else .7, so=.95 if "JFK" in (p, q) else .65, lc="round")
        hubs.update((p, q))
    for h in sorted(hubs):
        p, v = G(*_ll(h), clamp=False)
        if v:
            a.circle(p, 1.25, fill="#fff", stroke=a.AD, sw=.6)
    a.end()
    globe_shade(a, G, disc, op=.2)
    jfk, _ = G(*_ll("JFK"))
    a.circle(jfk, 4.2, fill="none", stroke=PAL["ink"], sw=.8)

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
    a.path(lens, fill=a.hatch(PAL["forest"], angle=-40, gap=3.2, sw=.35, op=.22))
    # bay along the lower right edge of the field
    a.path(smooth([(104, 160), (124, 140), (148, 128), (160, 150), (150, 170)], True), fill=PAL["water2"])
    a.path(smooth([(104, 160), (124, 140), (148, 128)]), cls="h", so=.5)
    tw = PAL["rock1"]
    g = f'<g transform="translate({n(C[0])} {n(C[1])}) rotate(-38)">'
    # parallel taxiways with angled high-speed exits, and the apron between the runways
    g += (f'<path d="M-46-13H44M-38 13H50M-30-13l7 5M-4-13l7 5M22-13l7 5M-24 13l7-5M4 13l7-5M30 13l7-5'
          f'M-46-13V13" stroke="{tw}" stroke-width="2.6" fill="none" stroke-linejoin="round"/>'
          f'<path d="M-46-13H44M-38 13H50M-46-13V13" stroke="{PAL["ink"]}" stroke-width=".25" stroke-dasharray="1 1" fill="none"/>'
          f'<rect x="-36" y="-8.5" width="62" height="17" rx="1.5" fill="{mix(PAL["rock1"], PAL["paper"], .35)}" stroke="{PAL["ink"]}" stroke-width=".35"/>')
    g += f'<g transform="translate(-2 -21)">{_runway(100, 7)}</g>'
    g += f'<g transform="translate(8 21)">{_runway(90, 6.4)}</g>'
    # terminal: a spine with three piers and aircraft nosed in at the gates, a control tower
    piers = "".join(f"M{x}-1.5V-6H{x + 3}V-1.5ZM{x} 1.5V6H{x + 3}V1.5Z" for x in (-22, -8, 6))
    g += (f'<path d="M-30-1.5H18V1.5H-30Z{piers}" fill="{PAL["paper"]}" stroke="{PAL["ink"]}" stroke-width=".45" stroke-linejoin="round"/>')
    for x in (-22, -8, 6):
        g += _plane(x - 3.4, -4.6, 0, .62) + _plane(x + 6.4, 4.6, 180, .62)
    g += f'<circle cx="22" cy="0" r="1.7" fill="{PAL["paper"]}" stroke="{PAL["ink"]}" stroke-width=".5"/>'
    g += "</g>"
    a.add(g)
    a.end()
    a.path(lens, cls="o")
    return a.render()


def ports():
    """A container terminal in section: a ship moored at the quay, its draught below the
    waterline, the dredged channel and the depth that decides which ships a port can take,
    a ship-to-shore gantry crane working the stacks, and the container yard behind."""
    a = Art("verm", seed=71)
    wl = 104                                   # waterline
    # sky-side background: yard on land at the right
    # ---- water body and seabed in section
    bed = [(0, 168), (30, 166), (60, 160), (90, 158), (190, 158), (206, 150), (214, 134), (214, 180), (0, 180)]
    a.path(P([(0, wl), (214, wl), (214, 180), (0, 180)], True), fill=a.lin([(0, PAL["water2"]), (1, PAL["water4"])], 0, 0, 0, 1))
    for k in range(1, 4):
        a.path(f"M0 {wl + k * 14}H214", stroke="#fff", sw=.4, so=.25)
    a.path(P(bed, True), fill=a.lin([(0, PAL["sand2"]), (1, PAL["clay"])], 0, 0, 0, 1))
    a.path(P(bed, True), fill=litho_dots(a))
    a.path(P(bed[:7]), cls="o", sw=.8)
    # dredged channel: the natural bed (dashed) was shallower
    a.path(smooth([(40, 150), (90, 146), (150, 144), (200, 142)]), cls="h", da="2 1.6", so=.8)
    # quay wall (concrete) and apron on piles at the right
    a.path(P([(214, 96), (320, 96), (320, 180), (214, 180)], True), fill=a.lin([(0, PAL["rock1"]), (1, PAL["rock"])], 0, 0, 0, 1))
    a.path(P([(214, 96), (320, 96), (320, 180), (214, 180)], True), fill=a.hatch(PAL["ink"], 45, 2.6, .3, .25))
    a.path(P([(214, 180), (214, 96), (320, 96)]), cls="o")
    a.path("M214 96h106", stroke=PAL["ink"], sw=1.6)
    # fenders
    a.path("".join(f"M{214 - 2} {wl - 4 + k * 8}h2v5h-2Z" for k in range(2)), fill=PAL["slated"])
    # ---- the ship: hull section with the red antifouling below the waterline
    hx0, hx1, deck, keel = 24, 208, 84, 146
    hull = f"M{hx0} {deck}H{hx1}V{keel - 6}Q{hx1} {keel} {hx1 - 8} {keel}H{hx0 + 18}Q{hx0 + 2} {keel} {hx0 - 4} {deck + 22}Z"
    a.path(hull, fill=PAL["slated"])
    a.add(f'<clipPath id="hw"><rect x="0" y="{wl - 3}" width="320" height="80"/></clipPath>')
    a.path(hull, fill=a.A, cp="url(#hw)")
    a.path(f"M{hx0 - 3} {wl - 3}H{hx1}", stroke="#fff", sw=.6, so=.7)
    a.path(hull, cls="o")
    a.path(P([(0, wl), (214, wl)]), stroke="#fff", sw=.8, so=.8)
    # translucent water over the submerged hull
    a.path(P([(0, wl), (214, wl), (214, keel + 2), (0, keel + 2)], True), fill=PAL["water3"], fo=.18)
    # bridge superstructure aft (left) and container bays
    a.path(f"M{hx0 + 6} {deck}v-34h18v34Z", fill="#fbfaf6", stroke=PAL["ink"], sw=.6)
    a.path("".join(f"M{hx0 + 8} {deck - 30 + k * 6}h14" for k in range(5)), stroke=PAL["slate"], sw=1.2)
    a.path(f"M{hx0 + 4} {deck - 34}h22v-3h-22Z", fill=PAL["slate2"])
    cols = [PAL["slate1"], PAL["sand2"], PAL["water3"], PAL["rock"], PAL["clay"], PAL["sage2"], mix(a.AL, PAL["rock1"], .5)]
    rnd = a.rng
    for bay in range(9):
        x = hx0 + 30 + bay * 16
        tiers = 3 + (bay % 4 == 1) + (2 < bay < 7)
        for tr in range(tiers):
            c = cols[rnd.randrange(len(cols))]
            a.path(f"M{x} {deck - (tr + 1) * 7}h15v7h-15Z", fill=c, stroke=PAL["ink"], sw=.35)
            a.path(f"M{x + 3} {deck - (tr + 1) * 7 + 1}v5M{x + 6} {deck - (tr + 1) * 7 + 1}v5M{x + 9} {deck - (tr + 1) * 7 + 1}v5M{x + 12} {deck - (tr + 1) * 7 + 1}v5",
                   stroke=PAL["ink"], sw=.25, so=.4)
    # ---- ship-to-shore gantry crane on rails at the quay edge
    cl, cr, top = 222, 258, 34
    a.path(f"M{cl} 96V{top}M{cr} 96V{top}M{cl} {top + 26}H{cr}M{cl} 96L{cr} {top + 26}M{cr} 96L{cl} {top + 26}", stroke=PAL["slated"], sw=1.6, fill="none")
    a.path(f"M120 {top}H300", stroke=PAL["slated"], sw=2.4)                    # boom over the ship
    a.path(f"M{cl} {top}L{cl + 18} {top - 22}L{cr} {top}M{cl + 18} {top - 22}L130 {top}M{cl + 18} {top - 22}L298 {top}", stroke=PAL["slated"], sw=.7, fill="none")
    a.path(f"M286 {top - 6}h14v12h-14Z", fill=PAL["slate1"], stroke=PAL["ink"], sw=.4)
    tx = 150
    a.path(f"M{tx - 5} {top - 2}h10v5h-10Z", fill=PAL["slate2"])
    a.path(f"M{tx - 3} {top + 3}V60M{tx + 3} {top + 3}V60", stroke=PAL["ink"], sw=.4)
    a.path(f"M{tx - 8} 60h16v7h-16Z", fill=cols[1], stroke=PAL["ink"], sw=.4)
    # ---- container yard on the land side
    for row in range(3):
        for k in range(3):
            x = 268 + k * 16
            a.path(f"M{x} {96 - (row + 1) * 7}h15v7h-15Z", fill=cols[(row * 3 + k) % len(cols)], stroke=PAL["ink"], sw=.35)
    # ---- channel depth and draught (accent dimension lines)
    dx = 196
    a.arrow([(dx, wl + 1.5), (dx, 156.5)], color=a.AD, w=.9, head=3.8, both=True, curve=False)
    a.path(f"M{dx - 4} 157.5h8M{dx - 4} {wl}h8", stroke=a.AD, sw=.6)
    a.arrow([(14, wl + 1.5), (14, keel - 1)], color=PAL["ink"], w=.7, head=3.2, both=True, curve=False)
    return a.render()


def litho_dots(a):
    ink = mix(PAL["sand2"], PAL["ink"], .4)
    return a.pattern(5, 4.4, f'<circle cx="1" cy="1" r=".4" fill="{ink}"/><circle cx="3.5" cy="3.2" r=".4" fill="{ink}"/>')


def railways():
    """A Japanese rail corridor in a hilly block: a high-speed line crossing the valley on a
    viaduct and entering a tunnel through the ridge, a conventional line beside the river, and
    stations drawn as on a route map; the cut face shows the tunnel bore in section."""
    from .kit import terrain, conifer
    a = Art("verm", seed=73)
    o = (10, 166)
    Wd, D = 214.0, 120.0

    def h(x, y):
        ridge = 34 * math.exp(-((x - 176) / 30) ** 2)
        hills = 12 * math.exp(-((x - 30) / 24) ** 2)
        valley = -4 * math.exp(-((x - 100) / 30) ** 2)
        return 8 + ridge + hills + valley + 2 * math.sin(y / 15) * math.cos(x / 23)

    def cover(x, y):
        z = h(x, y)
        if abs(x - 96 - (y - 60) * .3) < 5:
            return "river"
        return "forest" if z > 22 else ("paddy" if (int(x / 14) + int(y / 12)) % 2 else "field")
    cols = {"forest": PAL["forest"], "paddy": PAL["sage2"], "field": mix(PAL["sage1"], PAL["sand1"], .5), "river": PAL["water3"]}
    a.shadow(o[0] + 160, o[1] + 4, 150, 7, op=.2)
    surf = terrain(a, o, Wd, D, h, nx=30, ny=12, cover=cover, colours=cols, base=14,
                   strata=[(3, PAL["umber"]), (9, PAL["clay"]), (99, PAL["rock"])])
    rnd = a.rng
    tr = []
    for j in range(11, 0, -1):
        for i in range(26):
            x, y = i * 8.4 + rnd.uniform(-2, 2), j * 10.6 + rnd.uniform(-3, 3)
            if 2 < x < Wd - 2 and h(x, y) > 23:
                tr.append(surf(x, y))
    a.path("".join(conifer(p[0], p[1] + .5, 5.6, 3) for p in tr), fill=PAL["forestd"], stroke=mix(PAL["forestd"], PAL["ink"], .3), sw=.3)
    # tunnel bore in the right cut face
    ty = 46
    tz = 12
    pc = ob(Wd, ty, tz, o)
    a.path(f"M{n(pc[0] - 3)} {n(pc[1] + 3)}a3.4 4 0 0 1 6.8 -2.4v4Z", fill=PAL["slated"], stroke=PAL["ink"], sw=.5)
    # high-speed line: level, on a viaduct over the valley, into the tunnel portal
    line_z = 20
    xs = [x for x in range(0, 156, 4)]
    deck = [ob(x, ty, line_z, o) for x in xs]
    piers = ""
    for x in range(10, 150, 10):
        g = surf(x, ty)
        p = ob(x, ty, line_z, o)
        if g[1] - p[1] > 1.5:
            piers += f"M{n(p[0])} {n(p[1])}V{n(g[1])}"
    a.path(piers, stroke=PAL["rock"], sw=1.6, fill="none")
    a.path(piers, stroke=PAL["ink"], sw=.25, so=.6, fill="none")
    a.path(P(deck), stroke=PAL["rock1"], sw=4.6, fill="none")
    a.path(P(deck), stroke=PAL["ink"], sw=.4, so=.6, fill="none")
    a.path(P(deck), stroke=a.A, sw=1.4, fill="none")
    # tunnel portal where the line meets the ridge
    pp = ob(156, ty, line_z, o)
    a.path(f"M{n(pp[0] - 3)} {n(pp[1] + 2)}a3.4 4.4 0 0 1 6.8 0Z", fill=PAL["slated"], stroke=PAL["ink"], sw=.5)
    a.path(P([pp, ob(Wd, ty, tz + 2, o)]), stroke=a.A, sw=.8, da="1.6 1.4", fill="none", so=.7)
    # the train on the viaduct: a long-nosed set
    t0, t1 = 26, 96
    body = [ob(x, ty, line_z + 1, o) for x in (t0, t1)]
    a.path(P([body[0], body[1]]), stroke=PAL["ink"], sw=5.6, lc="butt")
    a.path(P([body[0], body[1]]), stroke="#fbfaf6", sw=4.8, lc="butt")
    nose = ob(t1 + 8, ty, line_z + .4, o)
    a.path(P([body[1], nose]), stroke=PAL["ink"], sw=3.6, lc="round")
    a.path(P([body[1], nose]), stroke="#fbfaf6", sw=2.8, lc="round")
    a.path(P([ob(t0, ty, line_z + .3, o), nose]), stroke=PAL["slate2"], sw=.6)
    a.path("".join(P([ob(x, ty, line_z - .6, o), ob(x, ty, line_z + 2.6, o)]) for x in range(t0 + 12, t1, 12)), stroke=PAL["ink"], sw=.35, so=.6)
    # conventional line along the river, with two stations
    cl = [surf(x, 60 + (x - 96) * 1.2 + 10, .6) for x in range(40, 160, 6) if 0 < 60 + (x - 96) * 1.2 + 10 < D]
    a.path(P(cl), stroke=PAL["ink"], sw=1.1, fill="none")
    a.path(P(cl), stroke="#fff", sw=.5, da="2 2", fill="none")
    for p in (cl[3], cl[-4]):
        a.circle(p, 2.2, fill="#fff", stroke=PAL["ink"], sw=.8)
    st = ob(20, ty, line_z, o)
    a.circle(ob(24, ty, line_z, o), 2.6, fill="#fff", stroke=PAL["ink"], sw=.9)
    return a.render()
