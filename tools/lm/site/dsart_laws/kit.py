"""Drawing kit for the nature-journal set of Data page illustrations (see ../data_art_laws.py).

A page from a field journal, after the manner of John Muir Laws (imitating the approach, not any
one drawing): pen line first, transparent watercolour washes second, white paper left showing,
a main study with smaller zoom-in studies in circles or boxes, leader lines ending in dots,
coloured-pencil arrows, scale bars and size-comparison figures.

The cards are shown in 11 languages, so a page never carries words, letters or digits: notes are
replaced by leader lines, arrows, scale bars and at most three dot-numbered callouts.

Rendering model (cheap on purpose):
- two filter definitions per file: ``i`` (a faint feTurbulence/feDisplacementMap wobble for the
  pen) and ``w`` (ragged edges and pigment mottling for the washes);
- drawing calls append to one ordered list; consecutive calls of the same kind are wrapped in one
  filtered group, so a page has only a handful of filtered groups;
- a small tiled pattern of specks gives the paper grain without a filter.

Standard library only; deterministic (random.Random with fixed seeds, fixed turbulence seeds).
"""
from __future__ import annotations

import json
import math
import re
import random
from contextlib import contextmanager
from pathlib import Path

W, H = 320, 180
REPO = Path(__file__).resolve().parents[4]

PAPER = "#f7f2e6"
INK = "#2b2620"
INK2 = "#5a5047"         # pencil / secondary line
BLUE_PENCIL = "#7fa6cf"  # construction lines
RED_PENCIL = "#c4513a"   # process and motion arrows
BLUE_ARROW = "#4f7fb0"

# A small watercolour palette (muted, as in a travel paint box).
C = {
    "ultra": "#5d73b0", "cerul": "#76a7c9", "sky": "#a9c8de", "skyp": "#d6e5ee",
    "sap": "#7f9a4f", "sapl": "#b5c58e", "olive": "#8b8a4e", "hooker": "#55703f",
    "ochre": "#d4a24a", "ochrel": "#e8cc8a", "lemon": "#e9d46c", "sienna": "#c27f4a",
    "bsienna": "#a2593a", "umber": "#6d4b36", "umberl": "#a88a6c", "sand": "#dcc59a",
    "payne": "#5a6370", "grey": "#9a9a96", "greyl": "#c9c6bd", "slate": "#7c8792",
    "red": "#cf5236", "crimson": "#b2414a", "orange": "#e0873a", "rose": "#d98c86",
    "violet": "#8a77a6", "teal": "#4f8f8a", "night": "#2f3b5c", "white": "#fbf8f0",
}


# ------------------------------------------------------------------ numbers and paths
def n(v: float) -> str:
    """One decimal, no trailing zero, no leading zero."""
    s = f"{v:.1f}"
    if s.endswith(".0"):
        s = s[:-2]
    if s in ("-0", ""):
        return "0"
    if s.startswith("0."):
        return s[1:]
    if s.startswith("-0."):
        return "-" + s[2:]
    return s


def _q(p):
    return (round(p[0] * 10), round(p[1] * 10))


def P(pts, close=False) -> str:
    """Polyline: absolute move, then relative lines."""
    q, last = [], None
    for p in pts:
        k = _q(p)
        if k != last:
            q.append(k)
            last = k
    if not q:
        return ""
    d = f"M{n(q[0][0] / 10)} {n(q[0][1] / 10)}"
    if len(q) > 1:
        rel = []
        for (x0, y0), (x1, y1) in zip(q, q[1:]):
            rel += [n((x1 - x0) / 10), n((y1 - y0) / 10)]
        d += "l" + " ".join(rel).replace(" -", "-")
    return d + ("Z" if close else "")


def S(pts, close=False, t=0.5) -> str:
    """Catmull-Rom spline through pts, as relative cubic Bezier segments."""
    pts = list(pts)
    if len(pts) < 3:
        return P(pts, close)
    m = len(pts)
    cur = _q(pts[0])
    d = f"M{n(cur[0] / 10)} {n(cur[1] / 10)}c"
    nums = []
    for i in (range(m) if close else range(m - 1)):
        p0 = pts[(i - 1) % m] if (close or i > 0) else pts[i]
        p1, p2 = pts[i], pts[(i + 1) % m]
        p3 = pts[(i + 2) % m] if (close or i + 2 < m) else p2
        c1 = _q((p1[0] + (p2[0] - p0[0]) * t / 3, p1[1] + (p2[1] - p0[1]) * t / 3))
        c2 = _q((p2[0] - (p3[0] - p1[0]) * t / 3, p2[1] - (p3[1] - p1[1]) * t / 3))
        e = _q(p2)
        for qq in (c1, c2, e):
            nums += [n((qq[0] - cur[0]) / 10), n((qq[1] - cur[1]) / 10)]
        cur = e
    return d + " ".join(nums).replace(" -", "-") + ("Z" if close else "")


def scallop(pts, bulge=0.3, close=False) -> str:
    """Bulging arcs between successive points (cumulus outlines, leaf clumps); bulge to the left
    of the direction of travel (outward for a clockwise outline in screen coordinates)."""
    d = f"M{n(pts[0][0])} {n(pts[0][1])}"
    seq = list(zip(pts, pts[1:] + ([pts[0]] if close else [])))
    for (x0, y0), (x1, y1) in seq:
        mx, my = (x0 + x1) / 2, (y0 + y1) / 2
        dx, dy = x1 - x0, y1 - y0
        d += f"Q{n(mx + dy * bulge)} {n(my - dx * bulge)} {n(x1)} {n(y1)}"
    return d + ("Z" if close else "")


def ell_pts(cx, cy, rx, ry, k=24, a0=0.0, a1=2 * math.pi, rot=0.0):
    out = []
    cr, sr = math.cos(rot), math.sin(rot)
    for i in range(k + 1):
        a = a0 + (a1 - a0) * i / k
        x, y = rx * math.cos(a), ry * math.sin(a)
        out.append((cx + x * cr - y * sr, cy + x * sr + y * cr))
    return out


def circ(cx, cy, r) -> str:
    return (f"M{n(cx - r)} {n(cy)}a{n(r)} {n(r)} 0 1 0 {n(2 * r)} 0"
            f"a{n(r)} {n(r)} 0 1 0 {n(-2 * r)} 0Z")


def ell(cx, cy, rx, ry) -> str:
    return (f"M{n(cx - rx)} {n(cy)}a{n(rx)} {n(ry)} 0 1 0 {n(2 * rx)} 0"
            f"a{n(rx)} {n(ry)} 0 1 0 {n(-2 * rx)} 0Z")


def rect(x, y, w, h) -> str:
    return f"M{n(x)} {n(y)}h{n(w)}v{n(h)}h{n(-w)}Z"


def lerp(a, b, t):
    return a + (b - a) * t


def lerp2(p, q, t):
    return (p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t)


def mix(c1: str, c2: str, t: float) -> str:
    a = [int(c1[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(c2[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(x + (y - x) * t):02x}" for x, y in zip(a, b))


def jitter(pts, amp, rng):
    return [(x + rng.uniform(-amp, amp), y + rng.uniform(-amp, amp)) for x, y in pts]


def hatch_lines(poly, angle_deg, gap, rng=None, inset=0.0):
    """Parallel hatching clipped to a polygon (even-odd); returns a list of segments."""
    a = math.radians(angle_deg)
    ux, uy = math.cos(a), math.sin(a)          # hatch direction
    vx, vy = -uy, ux                           # step direction
    proj = [x * vx + y * vy for x, y in poly]
    lo, hi = min(proj), max(proj)
    segs = []
    k = lo + gap / 2
    m = len(poly)
    while k < hi:
        hits = []
        for i in range(m):
            (x0, y0), (x1, y1) = poly[i], poly[(i + 1) % m]
            p0, p1 = x0 * vx + y0 * vy, x1 * vx + y1 * vy
            if (p0 - k) * (p1 - k) < 0:
                t = (k - p0) / (p1 - p0)
                x, y = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
                hits.append((x * ux + y * uy, x, y))
        hits.sort()
        for j in range(0, len(hits) - 1, 2):
            (s0, x0, y0), (s1, x1, y1) = hits[j], hits[j + 1]
            if s1 - s0 > 2 * inset + 0.6:
                x0, y0 = x0 + ux * inset, y0 + uy * inset
                x1, y1 = x1 - ux * inset, y1 - uy * inset
                if rng is not None:
                    sh = rng.uniform(0, 0.25) * (s1 - s0 - 2 * inset)
                    x1, y1 = x1 - ux * sh, y1 - uy * sh
                segs.append(((x0, y0), (x1, y1)))
        k += gap
    return segs


def segs_d(segs) -> str:
    return "".join(P(s) for s in segs)


# ------------------------------------------------------------------ geodata
_GEO = {}


def countries():
    if "c" not in _GEO:
        data = json.loads((REPO / "assets" / "examples" / "countries.geojson").read_text(encoding="utf-8"))
        rings = []
        _GEO["cont"] = {f["properties"].get("iso_a3"): f["properties"].get("continent") for f in data["features"]}
        for f in data["features"]:
            g = f["geometry"]
            polys = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
            for poly in polys:
                rings.append((f["properties"].get("iso_a3"), poly[0]))
        _GEO["c"] = rings
    return _GEO["c"]


def continent(iso):
    countries()
    return _GEO["cont"].get(iso)


def cities():
    if "t" not in _GEO:
        data = json.loads((REPO / "assets" / "examples" / "cities.geojson").read_text(encoding="utf-8"))
        _GEO["t"] = [(f["geometry"]["coordinates"][0], f["geometry"]["coordinates"][1],
                      f["properties"].get("population") or 0, bool(f["properties"].get("capital")))
                     for f in data["features"]]
    return _GEO["t"]


def country_rings(iso):
    return [ring for i, ring in countries() if i == iso]


def inside(pt, rings):
    """Even-odd point-in-polygon over several rings (lon/lat or screen, as long as they agree)."""
    x, y = pt
    c = False
    for ring in rings:
        m = len(ring)
        for i in range(m):
            (x0, y0), (x1, y1) = ring[i], ring[(i + 1) % m]
            if (y0 > y) != (y1 > y) and x < (x1 - x0) * (y - y0) / (y1 - y0) + x0:
                c = not c
    return c


class Equi:
    """Equirectangular (cos-latitude scaled) map of a lon/lat box into a screen box."""

    def __init__(self, lon0, lon1, lat0, lat1, x, y, w, h=None):
        self.lon0, self.lon1, self.lat0, self.lat1 = lon0, lon1, lat0, lat1
        k = math.cos(math.radians((lat0 + lat1) / 2))
        self.sx = w / ((lon1 - lon0) * k)
        self.k = k
        self.x, self.y = x, y
        self.h = h if h is not None else (lat1 - lat0) * self.sx

    def __call__(self, lon, lat):
        return (self.x + (lon - self.lon0) * self.k * self.sx, self.y + (self.lat1 - lat) * self.sx)

    def land(self, pad=4.0, tol=0.6, only=None):
        """Path data of the country rings that touch the box, simplified to tol px."""
        out = []
        shifts = [0.0] + ([360.0] if self.lon1 > 180 else []) + ([-360.0] if self.lon0 < -180 else [])
        for iso, ring0 in countries():
            if only and iso not in only:
                continue
            for sh in shifts:
                ring = [(p[0] + sh, p[1]) for p in ring0] if sh else ring0
                out.append(self._ring(ring, pad, tol))
        return "".join(out)

    def _ring(self, ring, pad, tol):
        if True:
            lons = [p[0] for p in ring]
            lats = [p[1] for p in ring]
            if max(lons) < self.lon0 - pad or min(lons) > self.lon1 + pad:
                return ""
            if max(lats) < self.lat0 - pad or min(lats) > self.lat1 + pad:
                return ""
            pts = simplify([self(lo, la) for lo, la in ring], tol)
            return P(pts, True) if len(pts) >= 3 and _span(pts) >= 1.2 else ""


class Ortho:
    """Orthographic globe centred on (lon0, lat0), radius r at (cx, cy)."""

    def __init__(self, lon0, lat0, cx, cy, r):
        self.l0, self.p0 = math.radians(lon0), math.radians(lat0)
        self.cx, self.cy, self.r = cx, cy, r

    def __call__(self, lon, lat):
        l, p = math.radians(lon), math.radians(lat)
        cosc = math.sin(self.p0) * math.sin(p) + math.cos(self.p0) * math.cos(p) * math.cos(l - self.l0)
        x = math.cos(p) * math.sin(l - self.l0)
        y = math.cos(self.p0) * math.sin(p) - math.sin(self.p0) * math.cos(p) * math.cos(l - self.l0)
        return (self.cx + self.r * x, self.cy - self.r * y, cosc >= 0)

    def land(self, tol=0.5):
        out = []
        for _, ring in countries():
            pts, vis = [], False
            for lo, la in ring:
                x, y, v = self(lo, la)
                if v:
                    vis = True
                    pts.append((x, y))
                else:   # push hidden points onto the limb so the ring closes along it
                    dx, dy = x - self.cx, y - self.cy
                    d = math.hypot(dx, dy) or 1
                    pts.append((self.cx + dx / d * self.r, self.cy + dy / d * self.r))
            if vis:
                pts = simplify(pts, tol)
                if len(pts) >= 3 and _span(pts) >= 1.6:
                    out.append(P(pts, True))
        return "".join(out)

    def line(self, lonlats):
        """Visible parts of a lon/lat polyline, as a list of point lists."""
        runs, cur = [], []
        for lo, la in lonlats:
            x, y, v = self(lo, la)
            if v:
                cur.append((x, y))
            elif cur:
                runs.append(cur)
                cur = []
        if cur:
            runs.append(cur)
        return runs


def _span(pts):
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return max(max(xs) - min(xs), max(ys) - min(ys))


def simplify(pts, tol):
    """Radial-distance simplification (cheap and good enough at card size)."""
    if not pts:
        return pts
    out = [pts[0]]
    for p in pts[1:]:
        if math.hypot(p[0] - out[-1][0], p[1] - out[-1][1]) >= tol:
            out.append(p)
    return out


_NUM = re.compile(r"[MmLlHhVvCcQqAaZz]|-?(?:\d+\.?\d*|\.\d+)")
_ARGS = {"m": 2, "l": 2, "h": 1, "v": 1, "c": 6, "q": 4, "a": 7, "z": 0}


def path_bbox(d):
    """Approximate bounding box of path data (end and control points; arcs padded by their radii)."""
    toks = _NUM.findall(d)
    x = y = sx = sy = 0.0
    xs, ys = [], []
    i, cmd = 0, None
    while i < len(toks):
        t = toks[i]
        if t.isalpha():
            cmd = t
            i += 1
            if cmd in "Zz":
                x, y = sx, sy
                continue
        k = _ARGS[cmd.lower()]
        a = [float(v) for v in toks[i:i + k]]
        i += k
        rel = cmd.islower()
        c = cmd.lower()
        if c in "ml":
            x, y = (x + a[0], y + a[1]) if rel else (a[0], a[1])
            if c == "m":
                sx, sy = x, y
                cmd = "l" if rel else "L"
            xs.append(x)
            ys.append(y)
        elif c == "h":
            x = x + a[0] if rel else a[0]
            xs.append(x)
            ys.append(y)
        elif c == "v":
            y = y + a[0] if rel else a[0]
            xs.append(x)
            ys.append(y)
        elif c in "cq":
            pts = [(a[j], a[j + 1]) for j in range(0, k, 2)]
            for px, py in pts:
                xs.append(x + px if rel else px)
                ys.append(y + py if rel else py)
            x, y = xs[-1], ys[-1]
        elif c == "a":
            r = max(a[0], a[1])
            nx, ny = (x + a[5], y + a[6]) if rel else (a[5], a[6])
            for px, py in ((x, y), (nx, ny)):
                xs += [px - r, px + r]
                ys += [py - r, py + r]
            x, y = nx, ny
    if not xs:
        return None
    return min(xs), min(ys), max(xs), max(ys)


# ------------------------------------------------------------------ the page
class Page:
    """One journal page. Drawing calls append to an ordered list (see the module docstring)."""

    def __init__(self, seed: int):
        self.seed = seed
        self.rng = random.Random(seed)
        self.items: list[tuple[str, str]] = []
        self.defs: list[str] = []
        self._ids = 0

    # ---- low level
    def _add(self, kind, s):
        self.items.append((kind, s))

    def uid(self, p="c"):
        self._ids += 1
        return f"{p}{self._ids}"

    def raw(self, s):
        """Unfiltered SVG (paper fills of insets, crisp marks)."""
        self._add("r", s)

    # ---- washes (watercolour layer)
    def wash(self, d, color, op=0.75, edge=0.55, ew=0.7):
        """A transparent wash with a slightly darker drying edge."""
        e = f' stroke="{mix(color, INK, 0.25)}" stroke-width="{n(ew)}" stroke-opacity="{n2(edge * op)}"' if edge else ""
        self._add("w", f'<path d="{d}" fill="{color}" fill-opacity="{n2(op)}"{e}/>')

    def land(self, d, color, op=0.85, sw=1.0):
        """A flat wash for maps: fill and a same-colour stroke composited together, so neighbouring
        country rings leave no slivers between them after the wash filter displaces their edges."""
        self._add("w", f'<path d="{d}" fill="{color}" stroke="{color}" stroke-width="{n(sw)}" '
                       f'stroke-linejoin="round" opacity="{n2(op)}"/>')

    def wash_grad(self, d, gid, op=1.0):
        self._add("w", f'<path d="{d}" fill="url(#{gid})" fill-opacity="{n2(op)}"/>')

    def lingrad(self, stops, x1=0, y1=0, x2=0, y2=1):
        gid = self.uid("g")
        st = "".join(f'<stop offset="{n2(o)}" stop-color="{c}" stop-opacity="{n2(a)}"/>' for o, c, a in stops)
        self.defs.append(f'<linearGradient id="{gid}" x1="{n2(x1)}" y1="{n2(y1)}" x2="{n2(x2)}" y2="{n2(y2)}">{st}</linearGradient>')
        return gid

    def radgrad(self, stops, cx=0.5, cy=0.5, r=0.5):
        gid = self.uid("g")
        st = "".join(f'<stop offset="{n2(o)}" stop-color="{c}" stop-opacity="{n2(a)}"/>' for o, c, a in stops)
        self.defs.append(f'<radialGradient id="{gid}" cx="{n2(cx)}" cy="{n2(cy)}" r="{n2(r)}">{st}</radialGradient>')
        return gid

    def stroke_wash(self, d, color, w=3.0, op=0.6):
        """A brush stroke of colour (rivers, roads, rays) in the wash layer."""
        self._add("w", f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{n(w)}" '
                       f'stroke-opacity="{n2(op)}" stroke-linecap="round" stroke-linejoin="round"/>')

    def paper(self, d):
        """Opaque paper (to knock out what is behind), with the paper grain on top."""
        self.raw(f'<path d="{d}" fill="{PAPER}"/><path d="{d}" fill="url(#pg)"/>')

    # ---- ink (pen layer)
    def ink(self, d, w=0.9, color=INK, op=None, dash=None, join="round"):
        o = f' stroke-opacity="{n2(op)}"' if op is not None else ""
        da = f' stroke-dasharray="{dash}"' if dash else ""
        self._add("i", f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{n2(w)}"{o}{da} '
                       f'stroke-linecap="round" stroke-linejoin="{join}"/>')

    def ink_fill(self, d, color=INK, op=None):
        o = f' fill-opacity="{n2(op)}"' if op is not None else ""
        self._add("i", f'<path d="{d}" fill="{color}"{o}/>')

    def hatch(self, poly, angle=-55, gap=1.6, w=0.35, color=INK, op=0.8, rng=True, inset=0.4):
        segs = hatch_lines(poly, angle, gap, self.rng if rng else None, inset)
        if segs:
            self.ink(segs_d(segs), w, color, op)

    def construct(self, d, w=0.55, op=0.55):
        """Light blue construction pencil, left visible under the finished drawing."""
        self.ink(d, w, BLUE_PENCIL, op)

    # ---- annotation (all wordless)
    def dot(self, x, y, r=1.1, color=INK):
        self.dots([(x, y)], r, color)

    def dots(self, pts, r, color=INK, op=None, layer="i"):
        """Round dots of radius r drawn as round-capped zero-length strokes (compact SVG)."""
        if not pts:
            return
        d = "".join(f"M{n(x)} {n(y)}h.01" for x, y in pts)
        o = f' stroke-opacity="{n2(op)}"' if op is not None else ""
        self._add(layer, f'<path d="{d}" stroke="{color}" stroke-width="{n2(2 * r)}"{o} stroke-linecap="round"/>')

    def dots_sized(self, pts, color, op=None, layer="i", step=0.2):
        """Dots of varying radius [(x, y, r)], grouped by radius rounded to step."""
        groups = {}
        for x, y, r in pts:
            groups.setdefault(round(r / step) * step, []).append((x, y))
        for r in sorted(groups, reverse=True):
            self.dots(groups[r], r, color, op, layer)

    def leader(self, a, b, dot_a=True, dot_b=False, w=0.5, bend=0.0):
        """A leader line from a (on the subject, with a dot) to b."""
        if bend:
            mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
            dx, dy = b[0] - a[0], b[1] - a[1]
            c = (mx - dy * bend, my + dx * bend)
            d = f"M{n(a[0])} {n(a[1])}Q{n(c[0])} {n(c[1])} {n(b[0])} {n(b[1])}"
        else:
            d = P([a, b])
        self.ink(d, w, INK, 0.85)
        if dot_a:
            self.dot(a[0], a[1], 1.05)
        if dot_b:
            self.dot(b[0], b[1], 1.05)

    def arrow(self, pts, color=RED_PENCIL, w=1.1, head=3.6, op=0.95, both=False, smooth=True):
        """A coloured-pencil arrow along pts with an open head."""
        d = S(pts) if (smooth and len(pts) > 2) else P(pts)
        self.ink(d, w, color, op)
        ends = [(pts[-2], pts[-1])] + ([(pts[1], pts[0])] if both else [])
        for p, q in ends:
            a = math.atan2(q[1] - p[1], q[0] - p[0])
            l = (q[0] - head * math.cos(a - 0.45), q[1] - head * math.sin(a - 0.45))
            r = (q[0] - head * math.cos(a + 0.45), q[1] - head * math.sin(a + 0.45))
            self.ink(P([l, q, r]), w, color, op)

    def scalebar(self, x, y, length, segs=4, h=2.2, vertical=False):
        """Alternating black/white bar with end ticks; no numbers."""
        step = length / segs
        for i in range(segs):
            if vertical:
                r = rect(x - h / 2, y - (i + 1) * step, h, step)
            else:
                r = rect(x + i * step, y - h / 2, step, h)
            if i % 2 == 0:
                self.ink_fill(r, INK, 0.85)
        if vertical:
            self.ink(rect(x - h / 2, y - length, h, length), 0.5)
            self.ink(P([(x - h, y), (x + h, y)]) + P([(x - h, y - length), (x + h, y - length)]), 0.5)
        else:
            self.ink(rect(x, y - h / 2, length, h), 0.5)
            self.ink(P([(x, y - h), (x, y + h)]) + P([(x + length, y - h), (x + length, y + h)]), 0.5)

    def callout(self, x, y, k, r=3.4):
        """A small circled callout numbered with k dots (never a digit or letter)."""
        self.paper(circ(x, y, r))
        self.ink(circ(x, y, r), 0.55)
        offs = {1: [(0, 0)], 2: [(-1.1, 0), (1.1, 0)], 3: [(-1.2, .7), (1.2, .7), (0, -1.1)]}[k]
        for dx, dy in offs:
            self.dot(x + dx, y + dy, 0.65)

    def person(self, x, y, h=10.0, color=INK):
        """A standing figure for scale: feet at (x, y)."""
        s = h / 10
        head = circ(x, y - 9.1 * s, 0.9 * s)
        body = P([(x - 1.1 * s, y - 7.8 * s), (x + 1.1 * s, y - 7.8 * s), (x + 1.3 * s, y - 4.4 * s),
                  (x + 0.8 * s, y - 4.4 * s), (x + 0.75 * s, y), (x + 0.1 * s, y), (x, y - 3.6 * s),
                  (x - 0.1 * s, y), (x - 0.75 * s, y), (x - 0.8 * s, y - 4.4 * s), (x - 1.3 * s, y - 4.4 * s)], True)
        self.ink_fill(head + body, color, 0.85)

    def ground(self, x0, x1, y, color, h=6.0, op=0.45, seed=0):
        """A quick horizontal wash for a subject to stand on, fading at both ends."""
        rng = random.Random(self.seed * 31 + seed)
        k = 10
        top = [(lerp(x0, x1, i / k), y - h * (0.35 + 0.65 * math.sin(math.pi * i / k)) + rng.uniform(-0.8, 0.8))
               for i in range(k + 1)]
        bot = [(lerp(x1, x0, i / k), y + h * 0.35 * math.sin(math.pi * i / k) + rng.uniform(-0.5, 0.5))
               for i in range(k + 1)]
        self.wash(S(top + bot, True), color, op, edge=0)

    def vignette(self, x0, y0, x1, y1, color, op=0.5, amp=4.0, seed=0, edge=0.0, k=9):
        """A wash over a box whose borders wander, so it fades out as a journal vignette does."""
        rng = random.Random(self.seed * 13 + seed)
        pts = []
        for (ax, ay), (bx, by) in (((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))):
            for i in range(k):
                t = i / k
                pts.append((lerp(ax, bx, t) + rng.uniform(-amp, amp), lerp(ay, by, t) + rng.uniform(-amp, amp)))
        self.wash(S(pts, True), color, op, edge=edge)

    def grass(self, x0, x1, y, k=18, hgt=3.0, seed=0, w=0.45):
        rng = random.Random(self.seed * 17 + seed)
        segs = []
        for i in range(k):
            x = lerp(x0, x1, (i + rng.random()) / k)
            for _ in range(2):
                a = rng.uniform(-0.5, 0.5)
                hh = hgt * rng.uniform(0.5, 1.0)
                segs.append([(x, y), (x + math.sin(a) * hh, y - math.cos(a) * hh)])
        self.ink("".join(P(s) for s in segs), w, INK2, 0.8)

    # ---- insets
    @contextmanager
    def clipped(self, d):
        """Draw inside an arbitrary path (no background, no frame)."""
        cid = self.uid("k")
        self.defs.append(f'<clipPath id="{cid}"><path d="{d}"/></clipPath>')
        self._add("g+", f'<g clip-path="url(#{cid})">')
        yield
        self._add("g-", "</g>")

    @contextmanager
    def inset_circle(self, cx, cy, r, ring=0.8, bg=True):
        cid = self.uid("k")
        self.defs.append(f'<clipPath id="{cid}"><path d="{circ(cx, cy, r)}"/></clipPath>')
        if bg:
            self.paper(circ(cx, cy, r))
        self._add("g+", f'<g clip-path="url(#{cid})">')
        yield
        self._add("g-", "</g>")
        self.ink(circ(cx, cy, r), ring)

    @contextmanager
    def inset_box(self, x, y, w, h, ring=0.8, bg=True):
        cid = self.uid("k")
        self.defs.append(f'<clipPath id="{cid}"><path d="{rect(x, y, w, h)}"/></clipPath>')
        if bg:
            self.paper(rect(x, y, w, h))
        self._add("g+", f'<g clip-path="url(#{cid})">')
        yield
        self._add("g-", "</g>")
        # hand-drawn frame: corners overshoot a little, as a pen frame does
        o = 1.4
        self.ink(P([(x - o, y), (x + w + o * 0.6, y)]) + P([(x + w, y - o * 0.5), (x + w, y + h + o)]) +
                 P([(x + w + o * 0.4, y + h), (x - o * 0.8, y + h)]) + P([(x, y + h + o * 0.6), (x, y - o)]), ring)

    # ---- output
    def svg(self) -> str:
        sd = 3 + self.seed % 7
        defs = (
            f'<filter id="i" x="-1%" y="-1%" width="102%" height="102%">'
            f'<feTurbulence type="fractalNoise" baseFrequency=".045" numOctaves="2" seed="{sd}"/>'
            f'<feDisplacementMap in="SourceGraphic" scale="1.5" xChannelSelector="R" yChannelSelector="G"/></filter>'
            f'<filter id="w" x="-2%" y="-2%" width="104%" height="104%">'
            f'<feTurbulence type="fractalNoise" baseFrequency=".06" numOctaves="3" seed="{sd + 11}" result="n"/>'
            f'<feDisplacementMap in="SourceGraphic" in2="n" scale="3.2" xChannelSelector="R" yChannelSelector="G" result="d"/>'
            f'<feColorMatrix in="n" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 -.7 0 1.25" result="m"/>'
            f'<feComposite in="d" in2="m" operator="in"/></filter>'
            f'<pattern id="pg" width="67" height="53" patternUnits="userSpaceOnUse">'
            f'<path d="M3 4h.5M29 2h.4M51 7h.6M14 13h.4M40 17h.5M61 22h.4M7 27h.6M24 31h.4M47 33h.5M33 44h.4'
            f'M57 46h.6M11 48h.5M19 39l.3.3M64 36l.3-.3M37 9l.3.3" stroke="#a8987a" stroke-width=".5" stroke-opacity=".28"/></pattern>'
        ) + "".join(self.defs)
        out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}">',
               f"<defs>{defs}</defs>",
               f'<rect width="{W}" height="{H}" fill="{PAPER}"/><rect width="{W}" height="{H}" fill="url(#pg)"/>']
        # Consecutive items of one kind share a filtered group. The filter region is a fraction of
        # the group's bounding box, which ignores stroke width and is empty for a lone straight
        # line, so each group gets an invisible rectangle 4 px larger than its geometry all round.
        runs = []
        for kind, s in self.items:
            if kind in ("g+", "g-") or not runs or runs[-1][0] != kind:
                runs.append((kind, [s]))
            else:
                runs[-1][1].append(s)
        dre = re.compile(r' d="([^"]+)"')
        for kind, parts in runs:
            if kind in ("w", "i"):
                boxes = [path_bbox(m) for p in parts for m in dre.findall(p)]
                boxes = [b for b in boxes if b]
                pad = ""
                if boxes:
                    x0 = min(b[0] for b in boxes) - 4
                    y0 = min(b[1] for b in boxes) - 4
                    x1 = max(b[2] for b in boxes) + 4
                    y1 = max(b[3] for b in boxes) + 4
                    pad = f'<rect x="{n(x0)}" y="{n(y0)}" width="{n(x1 - x0)}" height="{n(y1 - y0)}" fill="none"/>'
                out.append(f'<g filter="url(#{kind})">{pad}' + "".join(parts) + "</g>")
            else:
                out += parts
        out.append("</svg>")
        return "".join(out)


def n2(v: float) -> str:
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    if s.startswith("0."):
        s = s[1:]
    return "0" if s in ("-0", "") else s
