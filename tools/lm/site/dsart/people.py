"""Cities and people: world cities, life expectancy, population grid."""
import math

from .kit import (PAL, Art, P, smooth, ob, mix, n, lerp, lerp2, box_faces, Ortho, globe, globe_shade,
                  countries, cities, scallop)


def _box(a: Art, x0, x1, y0, y1, z0, z1, o, front, top, right, hair=True):
    """An oblique box spanning x0..x1, depth y0..y1, height z0..z1."""
    f = [ob(x0, y0, z1, o), ob(x1, y0, z1, o), ob(x1, y0, z0, o), ob(x0, y0, z0, o)]
    t = [ob(x0, y0, z1, o), ob(x0, y1, z1, o), ob(x1, y1, z1, o), ob(x1, y0, z1, o)]
    r = [ob(x1, y0, z1, o), ob(x1, y1, z1, o), ob(x1, y1, z0, o), ob(x1, y0, z0, o)]
    a.path(P(f, True) + "", fill=front)
    a.path(P(t, True), fill=top)
    a.path(P(r, True), fill=right)
    if hair:
        a.path(P(f, True) + P([t[1], t[2], r[2]]), stroke=PAL["ink"], sw=.35, so=.7, fill="none", lj="round")


def _slab(a, o, Wd, D, th, top):
    f, t, r = box_faces(0, Wd, -th, 0, D, o)
    a.path(P(t, True), fill=top)
    a.path(P(f, True), fill=PAL["soil"])
    a.path(P(r, True), fill=PAL["umber"])
    return f, t, r


def _outline(a, f, t, r):
    a.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="o", sw=.9)


def _cities_art_seg_dist(p, a, b):
    """Plan distance from point p to segment ab."""
    ax, ay = a
    bx, by = b
    dx, dy = bx - ax, by - ay
    L2 = dx * dx + dy * dy or 1.0
    t_ = max(0.0, min(1.0, ((p[0] - ax) * dx + (p[1] - ay) * dy) / L2))
    return math.hypot(p[0] - ax - t_ * dx, p[1] - ay - t_ * dy)


def _cities_art_box(a, x0, x1, y0, y1, h, o):
    """A building: an oblique box on the ground, every visible face edged so the roof reads."""
    f = [ob(x0, y0, h, o), ob(x1, y0, h, o), ob(x1, y0, 0, o), ob(x0, y0, 0, o)]
    t = [ob(x0, y0, h, o), ob(x0, y1, h, o), ob(x1, y1, h, o), ob(x1, y0, h, o)]
    r = [ob(x1, y0, h, o), ob(x1, y1, h, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)]
    a.path(P(f, True), fill=PAL["paper2"])
    a.path(P(t, True), fill="#fff")
    a.path(P(r, True), fill=PAL["rock1"])
    a.path(P(f, True) + P(t, True) + P(r, True), stroke=PAL["ink"], sw=.4, so=.75, fill="none", lj="round")


def cities_art():
    """A settled river plain: one city and a few smaller towns joined by roads. Each settlement
    sits on a disc whose area is proportional to its population (the rank-size pattern a world
    cities layer shows), and its buildings rise towards its core, tallest in the city."""
    a = Art("cobalt", seed=61)
    o = (12, 136)
    Wd, D = 150.0, 214.0
    a.shadow(o[0] + 150, o[1] + 3, 150, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, 6, PAL["sage1"])
    rnd = a.rng
    # the river, in plan units (x across, y in depth), crossing the plain
    river = [(0, 128), (26, 138), (52, 140), (80, 126), (108, 112), (130, 106), (150, 100)]
    # run the drawn line on past both side edges; the top-face clip trims it flush with them
    rs = [ob(x, y, 0, o) for x, y in [(-14, 120)] + river + [(166, 95)]]
    cp = a.clip(P(t, True))  # keep the river on the top face, off the block edges
    a.path(smooth(rs), stroke=PAL["water3"], sw=4.2, fill="none", lc="butt", cp=cp)
    a.path(smooth(rs), stroke=PAL["water2"], sw=2.4, fill="none", lc="butt", cp=cp)
    # settlements (x, y, population weight): the city, two towns, two villages
    places = [(58, 86, 1.0), (118, 162, .42), (122, 32, .3), (22, 198, .15), (22, 26, .1)]
    roads = [(0, 1), (0, 2), (0, 3), (0, 4)]
    for i, j in roads:
        p, q = places[i], places[j]
        L = math.hypot(q[0] - p[0], q[1] - p[1])
        ux, uy = (q[0] - p[0]) / L, (q[1] - p[1]) / L
        rp, rq = 26 * math.sqrt(p[2]), 26 * math.sqrt(q[2])  # roads end at each settlement's rim
        a.path(P([ob(p[0] + ux * rp, p[1] + uy * rp, 0, o), ob(q[0] - ux * rq, q[1] - uy * rq, 0, o)]), stroke="#fff",
               sw=1.4 + 1.2 * min(p[2], q[2]) ** .5, fill="none", lc="round", so=.95)
    # discs on the ground, area proportional to population
    for (x, y, w) in places:
        rr = 26 * math.sqrt(w)
        pts = [ob(x + rr * math.cos(t_), y + rr * math.sin(t_), 0, o) for t_ in [2 * math.pi * k / 48 for k in range(48)]]
        a.path(P(pts, True), fill=a.A, fo=.16, stroke=a.A, sw=.9, so=.85)
    # the block's edges go down before the buildings: towers standing on the plain hide its far edges
    _outline(a, f, t, r)
    # buildings on a regular grid of lots (no overlaps), off the river, taller towards each core
    boxes = []
    for (x, y, w) in places:
        rr = 26 * math.sqrt(w)
        pitch, s = (8.0, 6.0) if w > .5 else (7.0, 5.4)
        reach = max(pitch * .55, rr * .62)
        k = int(reach // pitch) + 1
        for i in range(-k, k + 1):
            for j in range(-k, k + 1):
                cx, cy = x + i * pitch, y + j * pitch
                d = math.hypot(i * pitch, j * pitch)
                if d > reach:
                    continue
                if min(_cities_art_seg_dist((cx, cy), river[m], river[m + 1]) for m in range(len(river) - 1)) < s + 3:
                    continue
                hgt = max(3.0, (5 + 62 * w) * math.exp(-(d / max(rr, 1)) ** 2 * 4) * rnd.uniform(.8, 1.05))
                boxes.append((cx - s / 2, cy - s / 2, s, hgt))
    for (x, y, s, hgt) in sorted(boxes, key=lambda b: (-b[1], b[0])):
        _cities_art_box(a, x, x + s, y, y + s, hgt, o)
    return a.render()


def _survival(x, k):
    """Share of a birth cohort still alive at age x; k blends 1850 (0) to 2023 (1) mortality."""
    infant = lerp(.28, .02, k)
    a_ = lerp(.008, .0004, k)
    b = lerp(.004, .00002, k)
    c = lerp(.075, .105, k)
    return (1 - infant * (1 - math.exp(-x / 1.2))) * math.exp(-a_ * x - b / c * (math.exp(c * x) - 1))


def _life_expectancy_cross(k, v=.5):
    """Age at which the survival curve for blend k falls to share v (bisection)."""
    lo, hi = 0.0, 105.0
    for _ in range(40):
        mid = (lo + hi) / 2
        if _survival(mid, k) > v:
            lo = mid
        else:
            hi = mid
    return (lo + hi) / 2


# Coarse present-day classes (period life expectancy at birth, about 2023): under ~68 years,
# ~68-77, and ~78 or more. Everything not listed is the middle class.
_LIFE_EXPECTANCY_HIGH = {
    "AUT", "BEL", "CHE", "CYP", "CZE", "DEU", "DNK", "ESP", "EST", "FIN", "FRA", "GBR", "GRC", "IRL",
    "ISL", "ITA", "LUX", "NLD", "NOR", "POL", "PRT", "SVN", "SWE", "HRV", "MLT",
    "ISR", "JPN", "KOR", "CHN", "AUS", "NZL", "CAN", "USA", "CHL", "CRI"}


_LIFE_EXPECTANCY_NORTH_AFRICA = {"EGY", "LBY", "TUN", "DZA", "MAR", "ESH"}


_LIFE_EXPECTANCY_LOW_OTHER = {"AFG", "PNG", "HTI"}


def _life_expectancy_land(G, geoms, simplify=.5):
    """Land path for a list of GeoJSON geometries (by feature, so unnamed-ISO areas such as
    Somaliland get their own class); back-side points are pushed to the limb as in Ortho.land."""
    from .kit import _thin
    out = []
    for g in geoms:
        polys = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
        for poly in polys:
            pts, vis = [], 0
            for lon, lat in poly[0]:
                q, v = G(lon, lat)
                vis += v
                pts.append(q)
            if vis == 0:
                continue
            pts = _thin(pts, simplify)
            if len(pts) >= 3:
                out.append(P(pts, True))
    return "".join(out)


def life_expectancy():
    """Survival curves (share of a birth cohort alive at each age) in the mid-nineteenth century
    and today: the curve has become nearly rectangular, so the age by which half have died (arrow)
    has more than doubled. Beside it, today's life expectancy by country on a globe (darker =
    longer lives): still shortest across sub-Saharan Africa."""
    a = Art("cobalt", seed=63)
    x0, x1, yb, yt = 22, 168, 150, 34
    sx = lambda age: lerp(x0, x1 - 4, age / 105)
    sy = lambda v: lerp(yb, yt + 6, v)
    ages = [i * .5 for i in range(0, 12)] + list(range(6, 106, 2))
    old = [(sx(t), sy(_survival(t, 0))) for t in ages]
    new = [(sx(t), sy(_survival(t, 1))) for t in ages]
    base = [(sx(105), yb), (x0, yb)]
    # years gained (between the curves) in pale accent, the 1850 area in a light accent
    a.path(smooth(new) + "L" + "L".join(f"{n(px)} {n(py)}" for px, py in base) + "Z", fill=a.AP, fo=.9)
    a.path(smooth(old) + "L" + "L".join(f"{n(px)} {n(py)}" for px, py in base) + "Z", fill=a.AL, fo=.45)
    a.path(smooth(old), stroke=PAL["ink2"], sw=.9, fill="none", lc="round")
    a.path(smooth(new), stroke=a.A, sw=1.6, fill="none", lc="round")
    # axes drawn over the fills so the curves sit on them
    a.path(P([(x0, yt), (x0, yb), (x1, yb)]), cls="o")
    # the median age at death moves from the old curve to the new one
    m0, m1 = sx(_life_expectancy_cross(0)), sx(_life_expectancy_cross(1))
    a.arrow([(m0, sy(.5)), (m1 - .6, sy(.5))], color=a.AD, w=1.1, head=5, curve=False)
    a.circle((m0, sy(.5)), 1.5, fill=a.AD)
    # ---- globe: present-day life expectancy by country in three steps, darker = longer
    G = Ortho(20, 18, 60, (244, 92))
    disc, cp = globe(a, G, land=False, grat=False)
    import json
    from .kit import REPO
    d = json.loads((REPO / "assets" / "examples" / "countries.geojson").read_text(encoding="utf-8"))
    cls = {0: [], 1: [], 2: []}
    for ft in d["features"]:
        pr = ft["properties"]
        iso, cont = pr.get("iso_a3"), pr.get("continent")
        if cont == "Antarctica":
            continue
        if iso in _LIFE_EXPECTANCY_HIGH:
            c = 2
        elif (cont == "Africa" and iso not in _LIFE_EXPECTANCY_NORTH_AFRICA) or iso in _LIFE_EXPECTANCY_LOW_OTHER:
            c = 0
        else:
            c = 1
        cls[c].append(ft["geometry"])
    cols = {0: mix(a.AP, a.AL, .55), 1: mix(a.AL, a.A, .5), 2: a.AD}
    a.group(cp=cp)
    for c in (0, 1, 2):
        a.path(_life_expectancy_land(G, cls[c]), fill=cols[c], stroke=cols[c], sw=.7, lj="round")
    a.end()
    globe_shade(a, G, disc, op=.2)
    return a.render()


def _pip(x, y, ring):
    ins = False
    for (x1, y1), (x2, y2) in zip(ring, ring[1:] + ring[:1]):
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1 + 1e-12) + x1:
            ins = not ins
    return ins


def _population_hu(t):
    """A point at fraction t along the Hu Line, Heihe to Tengchong. The line is straight on a
    Lambert conic map (standard parallels 25N and 47N), so it bows north-west in lon/lat."""
    f1, f2, l0 = math.radians(25), math.radians(47), math.radians(105)
    tn = lambda f: math.tan(math.pi / 4 + f / 2)
    nn = math.log(math.cos(f1) / math.cos(f2)) / math.log(tn(f2) / tn(f1))
    F = math.cos(f1) * tn(f1) ** nn / nn

    def fwd(lon, lat):
        rho, th = F / tn(math.radians(lat)) ** nn, nn * (math.radians(lon) - l0)
        return rho * math.sin(th), -rho * math.cos(th)

    (x1, y1), (x2, y2) = fwd(127.5, 50.2), fwd(98.5, 25.0)
    x, y = lerp(x1, x2, t), lerp(y1, y2, t)
    rho = math.hypot(x, y)
    return math.degrees(l0 + math.atan2(x, -y) / nn), math.degrees(2 * math.atan((F / rho) ** (1 / nn)) - math.pi / 2)


def _population_clip(ring, x0, x1, y0, y1):
    """Sutherland-Hodgman: a lon/lat ring clipped to the box x0..x1, y0..y1."""
    pts = list(ring)
    for k, v, keep in ((0, x0, 1), (0, x1, -1), (1, y0, 1), (1, y1, -1)):
        if not pts:
            break
        out = []
        for p, q in zip(pts, pts[1:] + pts[:1]):
            pin, qin = (p[k] - v) * keep >= 0, (q[k] - v) * keep >= 0
            if pin:
                out.append(p)
            if pin != qin:
                s = (v - p[k]) / (q[k] - p[k])
                out.append((p[0] + s * (q[0] - p[0]), p[1] + s * (q[1] - p[1])))
        pts = out
    return pts


def _population_density(lon, lat):
    """Approximate relative population density of China (0..1): Gaussian blobs on the real
    clusters plus a gentle west-to-east rise."""
    blobs = (  # lon, lat, amplitude, sigma (degrees)
        (120.8, 31.2, 1.00, 1.5),   # Yangtze Delta (Shanghai)
        (113.4, 22.9, .70, 1.3),    # Pearl River Delta (Guangzhou, Shenzhen)
        (116.8, 39.4, .62, 1.3),    # Beijing-Tianjin
        (115.4, 35.0, .58, 2.4),    # North China Plain (Hebei, Henan, Shandong)
        (105.0, 30.4, .74, 1.8),    # Sichuan Basin (Chengdu-Chongqing)
        (113.5, 29.5, .42, 2.6),    # middle Yangtze (Wuhan, Changsha)
        (118.8, 25.8, .26, 2.0),    # Fujian coast
        (124.5, 43.0, .26, 2.8),    # north-east plain
        (108.8, 34.3, .30, 1.6),    # Guanzhong (Xi'an)
        (110.0, 25.0, .22, 2.6),    # Guangxi, Hunan south
    )
    d = sum(amp * math.exp(-((lon - x) ** 2 + (lat - y) ** 2) / (2 * s * s)) for x, y, amp, s in blobs)
    d += .1 * min(1, max(0, (lon - 98) / 22)) * min(1, max(0, (46 - lat) / 8))
    return min(1.0, d)


def _population_prism(a, x0, x1, y0, y1, hgt, o, top):
    """One extruded grid cell: front, right and top faces with a fine edge."""
    f = [ob(x0, y0, hgt, o), ob(x1, y0, hgt, o), ob(x1, y0, 0, o), ob(x0, y0, 0, o)]
    t = [ob(x0, y0, hgt, o), ob(x0, y1, hgt, o), ob(x1, y1, hgt, o), ob(x1, y0, hgt, o)]
    r = [ob(x1, y0, hgt, o), ob(x1, y1, hgt, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)]
    if hgt > .6:
        a.path(P(f, True), fill=mix(top, PAL["ink"], .16))
        a.path(P(r, True), fill=mix(top, PAL["ink"], .34))
    a.path(P(t, True), fill=top)
    edge = P(t, True) + (P([f[1], f[2]]) if hgt > .6 else "")
    a.path(edge, stroke=mix(top, PAL["ink"], .5), sw=.3, fill="none", lj="round")


def population():
    """A population grid over China on a map slab: the country and its neighbours are laid flat
    on the slab top in the block's own oblique projection, and each ~2-degree land cell is
    extruded by population density, so a skyline rises in the east (the Yangtze Delta highest,
    then the Pearl River Delta, Beijing-Tianjin and the North China Plain, the Sichuan Basin) beyond the dashed Hu Line, Heihe to
    Tengchong, while the west stays nearly flat."""
    a = Art("amber", seed=65)
    o = (7, 136)
    Wd, D, th = 166.0, 216.0, 6
    lon0, lon1, lat0, lat1 = 71.0, 137.0, 14.0, 56.0
    sx, sy = Wd / (lon1 - lon0), D / (lat1 - lat0)
    M = lambda lon, lat: ((lon - lon0) * sx, (lat - lat0) * sy)
    g = lambda lon, lat, z=0: ob(*M(lon, lat), z, o)
    a.shadow(o[0] + 160, o[1] + 3, 150, 6, op=.18)
    f, t, r = _slab(a, o, Wd, D, th, PAL["water1"])
    lands = []
    for iso, ring in countries():
        xs, ys = [p[0] for p in ring], [p[1] for p in ring]
        if max(xs) < lon0 or min(xs) > lon1 or max(ys) < lat0 or min(ys) > lat1:
            continue
        c = _population_clip(ring, lon0, lon1, lat0, lat1)
        if len(c) > 2:
            lands.append((iso, ring, c))
    china = [rg for iso, rg, c in lands if iso == "CHN"]
    main = max(china, key=len)
    home = ("CHN", "TWN")   # the mainland, Hainan and Taiwan share the pale land tone
    for iso, rg, c in lands:   # neighbours first, then China on top
        if iso not in home:
            a.path(P([g(*p) for p in c], True), fill=mix(PAL["rock1"], PAL["sand2"], .3), stroke="#fff", sw=.35, lj="round")
    for iso, rg, c in lands:
        if iso in home:
            a.path(P([g(*p) for p in c], True), fill=mix(PAL["sand1"], PAL["white"], .55), stroke=PAL["ink"], sw=.7, so=.85, lj="round")
    # the grid: regular 2-degree cells, aligned so one centres on Shanghai, kept when at least
    # 45% of the cell is mainland China; every prism has the same footprint
    cs, gap = 2.0, .22
    cells = []
    for j in range(18):
        lat = 18.0 + j * cs
        for i in range(31):
            lon = 74.0 + i * cs
            s5 = [(lon + cs * (u + .5) / 5, lat + cs * (v + .5) / 5) for u in range(5) for v in range(5)]
            if sum(_pip(x, y, main) for x, y in s5) >= 11:
                cells.append((lon, lat, _population_density(lon + cs / 2, lat + cs / 2)))
    # the Hu Line on the ground, with a pale halo so it stays legible over the flat cells
    hu = smooth([g(*_population_hu(i / 12)) for i in range(13)])
    ramp = [a.AP, a.AL, a.A, a.AD]
    flat = [c for c in cells if c[2] < .2]
    tall = [c for c in cells if c[2] >= .2]
    for lon, lat, d in flat:   # the nearly empty west: flat tiles on the ground
        (x0, y0), (x1, y1) = M(lon + gap, lat + gap), M(lon + cs - gap, lat + cs - gap)
        q = [ob(x0, y0, 0, o), ob(x0, y1, 0, o), ob(x1, y1, 0, o), ob(x1, y0, 0, o)]
        a.path(P(q, True), fill=mix(a.AP, PAL["white"], .62 - 1.5 * d))
    for iso, rg, c in lands:   # China's coast and border again, crisp over the flat tiles
        if iso in home:
            a.path(P([g(*p) for p in c], True), fill="none", stroke=PAL["ink"], sw=1.0, so=.9, lj="round")
    a.path(hu, stroke="#fff", sw=3.0, so=.85, fill="none", lc="round")
    a.path(hu, stroke=PAL["ink"], sw=1.5, fill="none", da="3.4 2", lc="round")
    for q in ((127.5, 50.2), (98.5, 25.0)):
        a.circle(g(*q), 1.6, fill="#fff", stroke=PAL["ink"], sw=.6)
    _outline(a, f, t, r)   # before the prisms, so towers stand in front of the back edge
    for lon, lat, d in sorted(tall, key=lambda c: (-c[1], c[0])):
        hgt = 1.5 + 36 * d ** 1.45
        k = min(.999, d ** .9) * 3
        tone = mix(ramp[int(k)], ramp[int(k) + 1], k - int(k))
        (x0, y0), (x1, y1) = M(lon + gap, lat + gap), M(lon + cs - gap, lat + cs - gap)
        _population_prism(a, x0, x1, y0, y1, hgt, o, tone)
    return a.render()
