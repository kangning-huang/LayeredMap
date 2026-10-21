"""Shared drawing kit for the Data page illustrations (see ../data_art.py).

One visual system for all 34 plates:

- a warm light plate (the same in light and dark mode; data.css dims it in dark mode), lit from
  the top left;
- three line weights: OUT (1.1 px) for the main forms, HAIR (0.55 px) for detail, and ANN (0.8 px)
  for annotation arrows, all in one ink;
- muted earth, water, vegetation and sky tones from PAL, plus ONE accent hue per plate
  (vermilion, amber or cobalt, each with a dark, light and pale step);
- one oblique projection for block diagrams: depth recedes up and to the right at 30 degrees,
  foreshortened to 0.75 (ob());
- orthographic globes drawn from the bundled Natural Earth countries (globe()).

Only the standard library; everything is deterministic (random.Random with a fixed seed).
"""
from __future__ import annotations

import json
import math
import random
from pathlib import Path

W, H = 320, 180
REPO = Path(__file__).resolve().parents[4]

# ---------------------------------------------------------------- palette
PAL = {
    "paper": "#f7f4ee", "paper2": "#efebe2",
    "ink": "#262b31", "ink2": "#59616b", "ink3": "#8e959d", "rule": "#cfc8ba", "white": "#fdfcf9",
    # earth
    "sand1": "#eee6d6", "sand2": "#ddd0b6", "soil": "#cbb491", "clay": "#b59a78", "umber": "#8e785c",
    "rock1": "#c9c1b5", "rock": "#a69d91", "rockd": "#7d756c", "char": "#4b4641",
    # vegetation
    "sage1": "#e2e7d7", "sage2": "#bfcbab", "moss": "#93a782", "forest": "#647b5c", "forestd": "#4a5f4a",
    # water and sky
    "water1": "#e1ebee", "water2": "#bcd3db", "water3": "#8db1bf", "water4": "#5f899b", "water5": "#3f6677",
    "sky1": "#eaf0f2", "sky2": "#dbe5ea",
    # cloud and slate
    "cloud": "#eceded", "cloud2": "#cfd3d6", "slate1": "#aab2b9", "slate": "#87909a", "slate2": "#636c76",
    "slated": "#474f58",
}
ACCENTS = {
    "verm": {"a": "#c4452c", "ad": "#8f2f1c", "al": "#e39a86", "ap": "#f5d9cf"},
    "amber": {"a": "#d18d22", "ad": "#93610f", "al": "#eac16f", "ap": "#f7e7c4"},
    "cobalt": {"a": "#2f62c4", "ad": "#1f448f", "al": "#8daade", "ap": "#d9e3f4"},
}

OUT, HAIR, ANN = 1.1, 0.55, 0.8
# oblique depth vector per unit of depth: 30 degrees, foreshortened to 0.75
DX, DY = 0.75 * math.cos(math.radians(30)), -0.75 * math.sin(math.radians(30))


def n(v: float) -> str:
    """A number for SVG: one decimal, no trailing or leading zero."""
    s = f"{v:.1f}"
    if s.endswith(".0"):
        s = s[:-2]
    if s == "-0":
        return "0"
    if s.startswith("0."):
        return s[1:]
    if s.startswith("-0."):
        return "-" + s[2:]
    return s


def n2(v: float) -> str:
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def pt(p) -> str:
    return f"{n(p[0])} {n(p[1])}"


def P(pts, close=False) -> str:
    """Polyline path data (absolute move, then relative lines), dropping repeated points."""
    q, last = [], None
    for p in pts:
        k = (round(p[0] * 10), round(p[1] * 10))
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


def _r(p):
    return (round(p[0] * 10), round(p[1] * 10))


def smooth(pts, close=False, t=0.5) -> str:
    """Catmull-Rom spline through pts as relative cubic Bezier path data."""
    pts = list(pts)
    if len(pts) < 3:
        return P(pts, close)
    m = len(pts)
    cur = _r(pts[0])
    d = f"M{n(cur[0] / 10)} {n(cur[1] / 10)}c"
    nums = []
    rng = range(m) if close else range(m - 1)
    for i in rng:
        p0 = pts[(i - 1) % m] if (close or i > 0) else pts[i]
        p1 = pts[i]
        p2 = pts[(i + 1) % m]
        p3 = pts[(i + 2) % m] if (close or i + 2 < m) else p2
        c1 = _r((p1[0] + (p2[0] - p0[0]) * t / 3, p1[1] + (p2[1] - p0[1]) * t / 3))
        c2 = _r((p2[0] - (p3[0] - p1[0]) * t / 3, p2[1] - (p3[1] - p1[1]) * t / 3))
        e = _r(p2)
        for q in (c1, c2, e):
            nums += [n((q[0] - cur[0]) / 10), n((q[1] - cur[1]) / 10)]
        cur = e
    return d + " ".join(nums).replace(" -", "-") + ("Z" if close else "")


def ob(x, y=0.0, z=0.0, o=(0.0, 0.0)):
    """Oblique projection: x right, y depth (away), z up, from screen origin o."""
    return (o[0] + x + y * DX, o[1] - z + y * DY)


def lerp(a, b, t):
    return a + (b - a) * t


def lerp2(p, q, t):
    return (lerp(p[0], q[0], t), lerp(p[1], q[1], t))


def mix(c1: str, c2: str, t: float) -> str:
    """Blend two #rrggbb (or #rgb) colours."""
    c1, c2 = (("#" + "".join(ch * 2 for ch in c[1:])) if len(c) == 4 else c for c in (c1, c2))
    a = [int(c1[i:i + 2], 16) for i in (1, 3, 5)]
    b = [int(c2[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(lerp(x, y, t)):02x}" for x, y in zip(a, b))


def attrs(kw) -> str:
    out = []
    for k, v in kw.items():
        if v is None:
            continue
        k = {"cls": "class", "sw": "stroke-width", "op": "opacity", "fo": "fill-opacity",
             "so": "stroke-opacity", "da": "stroke-dasharray", "lc": "stroke-linecap",
             "lj": "stroke-linejoin", "cp": "clip-path", "fr": "fill-rule", "tf": "transform",
             "mask": "mask"}.get(k, k.replace("_", "-"))
        if isinstance(v, float):
            v = n2(v)
        out.append((k, v))
    # a class's CSS beats presentation attributes, so overrides on a classed element go in style
    if any(k == "class" for k, _ in out):
        sty = [f"{k}:{v}" for k, v in out if k in _STYLED]
        out = [(k, v) for k, v in out if k not in _STYLED] + ([("style", ";".join(sty))] if sty else [])
    return " ".join(f'{k}="{v}"' for k, v in out)


_STYLED = {"stroke", "stroke-width", "stroke-opacity", "stroke-dasharray", "fill", "stroke-linecap"}


STYLE = (
    "<style>"
    f".o{{fill:none;stroke:{PAL['ink']};stroke-width:{OUT};stroke-linejoin:round;stroke-linecap:round}}"
    f".h{{fill:none;stroke:{PAL['ink']};stroke-opacity:.5;stroke-width:{HAIR};stroke-linejoin:round;stroke-linecap:round}}"
    f".hw{{fill:none;stroke:#fff;stroke-opacity:.7;stroke-width:{HAIR};stroke-linejoin:round;stroke-linecap:round}}"
    f".an{{fill:none;stroke:{PAL['ink']};stroke-width:{ANN};stroke-linecap:round;stroke-linejoin:round}}"
    f".ah{{fill:{PAL['ink']}}}"
    "</style>"
)


class Art:
    """One plate: defs, body, and the shared background."""

    def __init__(self, accent: str, seed: int = 1, ground: tuple | None = None):
        self.acc = ACCENTS[accent]
        self.A, self.AD, self.AL, self.AP = (self.acc[k] for k in ("a", "ad", "al", "ap"))
        self.defs: list[str] = []
        self.body: list[str] = []
        self._n = 0
        self.rng = random.Random(seed)
        self.ground = ground

    # ---- defs
    def uid(self, p="g") -> str:
        self._n += 1
        return f"{p}{self._n}"

    def lin(self, stops, x1=0, y1=0, x2=0, y2=1, user=False) -> str:
        i = self.uid("l")
        u = ' gradientUnits="userSpaceOnUse"' if user else ""
        s = "".join(_stop(*st) for st in stops)
        self.defs.append(f'<linearGradient id="{i}" x1="{n2(x1)}" y1="{n2(y1)}" x2="{n2(x2)}" y2="{n2(y2)}"{u}>{s}</linearGradient>')
        return f"url(#{i})"

    def rad(self, stops, cx=0.5, cy=0.5, r=0.5, fx=None, fy=None, user=False) -> str:
        i = self.uid("r")
        u = ' gradientUnits="userSpaceOnUse"' if user else ""
        f = (f' fx="{n2(fx)}"' if fx is not None else "") + (f' fy="{n2(fy)}"' if fy is not None else "")
        s = "".join(_stop(*st) for st in stops)
        self.defs.append(f'<radialGradient id="{i}" cx="{n2(cx)}" cy="{n2(cy)}" r="{n2(r)}"{f}{u}>{s}</radialGradient>')
        return f"url(#{i})"

    def clip(self, d: str) -> str:
        i = self.uid("c")
        self.defs.append(f'<clipPath id="{i}"><path d="{d}"/></clipPath>')
        return f"url(#{i})"

    def hatch(self, color, angle=45, gap=2.4, sw=0.5, op=1.0) -> str:
        i = self.uid("p")
        self.defs.append(
            f'<pattern id="{i}" width="{n2(gap)}" height="{n2(gap)}" patternUnits="userSpaceOnUse" '
            f'patternTransform="rotate({n(angle)})"><path d="M0 0V{n2(gap)}" stroke="{color}" '
            f'stroke-width="{n2(sw)}" stroke-opacity="{n2(op)}"/></pattern>')
        return f"url(#{i})"

    def pattern(self, w, h, content: str, tf: str | None = None, bg: str | None = None) -> str:
        """A userSpace tile of size w x h holding raw SVG content."""
        i = self.uid("p")
        t = f' patternTransform="{tf}"' if tf else ""
        b = f'<rect width="{n2(w)}" height="{n2(h)}" fill="{bg}"/>' if bg else ""
        self.defs.append(f'<pattern id="{i}" width="{n2(w)}" height="{n2(h)}" patternUnits="userSpaceOnUse"{t}>{b}{content}</pattern>')
        return f"url(#{i})"

    def blur(self, sd=2.0) -> str:
        i = self.uid("f")
        self.defs.append(f'<filter id="{i}" x="-30%" y="-30%" width="160%" height="160%"><feGaussianBlur stdDeviation="{n2(sd)}"/></filter>')
        return f"url(#{i})"

    # ---- body
    def add(self, *s: str):
        self.body.extend(s)

    def path(self, d: str, **kw):
        self.body.append(f'<path d="{d}" {attrs(kw)}/>')

    def circle(self, c, r, **kw):
        self.body.append(f'<circle cx="{n(c[0])}" cy="{n(c[1])}" r="{n2(r)}" {attrs(kw)}/>')

    def ellipse(self, c, rx, ry, **kw):
        self.body.append(f'<ellipse cx="{n(c[0])}" cy="{n(c[1])}" rx="{n2(rx)}" ry="{n2(ry)}" {attrs(kw)}/>')

    def group(self, **kw):
        self.body.append(f"<g {attrs(kw)}>")

    def end(self):
        self.body.append("</g>")

    def dots(self, pts, r=0.6, color=None, op=None):
        """Many round dots as one stroked path (cheap stippling)."""
        if not pts:
            return
        d = "".join(f"M{pt(p)}h0" for p in pts)
        self.path(d, stroke=color or PAL["ink"], sw=2 * r, lc="round", fill="none", so=op)

    def shadow(self, cx, cy, rx, ry=None, op=0.18):
        """Soft contact shadow, lit from the top left (falls to the lower right)."""
        g = self.rad([(0, PAL["ink"], op), (1, PAL["ink"], 0)])
        self.ellipse((cx, cy), rx, ry or rx * 0.14, fill=g)

    def arrow(self, pts, color=None, w=ANN, head=4.2, curve=True, half=False, both=False, da=None, op=None):
        """A line or Catmull-Rom curve through pts with a filled head at the end."""
        color = color or PAL["ink"]
        pts = [tuple(p) for p in pts]
        heads = [(pts[-1], pts[-2])] + ([(pts[0], pts[1])] if both else [])
        line = list(pts)
        # pull the ends back so the stroke doesn't poke through the tip
        for idx, (tip, prev) in zip([-1, 0], heads):
            dx, dy = tip[0] - prev[0], tip[1] - prev[1]
            L = math.hypot(dx, dy) or 1
            back = min(head * 0.7, L * 0.6)
            line[idx] = (tip[0] - dx / L * back, tip[1] - dy / L * back)
        d = smooth(line) if (curve and len(line) > 2) else P(line)
        self.path(d, fill="none", stroke=color, sw=w, lc="round", lj="round", da=da, so=op)
        for tip, prev in heads:
            self.head(tip, prev, color, head, half, op)

    def head(self, tip, prev, color, size=4.2, half=False, op=None):
        dx, dy = tip[0] - prev[0], tip[1] - prev[1]
        L = math.hypot(dx, dy) or 1
        ux, uy = dx / L, dy / L
        bx, by = tip[0] - ux * size, tip[1] - uy * size
        px, py = -uy * size * 0.42, ux * size * 0.42
        if half:
            pts = [tip, (bx + px, by + py), (tip[0] - ux * size * 0.75, tip[1] - uy * size * 0.75)]
        else:
            pts = [tip, (bx + px, by + py), (tip[0] - ux * size * 0.72, tip[1] - uy * size * 0.72), (bx - px, by - py)]
        self.path(P(pts, True), fill=color, fo=op)

    def render(self) -> str:
        bg = self.lin([(0, "#faf8f3"), (1, PAL["paper2"])], 0, 0, 0.35, 1)
        defs = f"<defs>{''.join(self.defs)}</defs>"
        return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}">{STYLE}{defs}'
                f'<rect width="{W}" height="{H}" fill="{bg}"/>' + "".join(self.body) + "</svg>\n")


def _stop(o, c, op=None):
    if op is None:
        return f'<stop offset="{n2(o)}" stop-color="{c}"/>'
    return f'<stop offset="{n2(o)}" stop-color="{c}" stop-opacity="{n2(op)}"/>'


# ---------------------------------------------------------------- shapes
def cloud_path(cx, cy, w, h, lobes=5, seed=0, flat=True) -> str:
    """A cumuliform cloud as a row of arcs over a flat (or rounded) base."""
    r = random.Random(seed)
    xs = [cx - w / 2 + w * (i + 0.5) / lobes for i in range(lobes)]
    d = f"M{n(cx - w / 2)} {n(cy)}"
    prev = (cx - w / 2, cy)
    for i, x in enumerate(xs):
        top = cy - h * (0.55 + 0.45 * math.sin(math.pi * (i + 0.5) / lobes)) * (0.85 + 0.3 * r.random())
        nxt = (cx - w / 2 + w * (i + 1) / lobes, cy - (0 if i == lobes - 1 else h * 0.35 * r.random()))
        d += f"C{n(prev[0])} {n(top)} {n(nxt[0])} {n(top)} {n(nxt[0])} {n(nxt[1])}"
        prev = nxt
    d += "Z" if flat else f"Q{n(cx)} {n(cy + h * 0.25)} {n(cx - w / 2)} {n(cy)}Z"
    return d


def conifer(x, y, h, w=None) -> str:
    """A small conifer silhouette, base centre at (x, y)."""
    w = w or h * 0.42
    return (f"M{n(x)} {n(y - h)}L{n(x + w * 0.32)} {n(y - h * 0.62)}L{n(x + w * 0.2)} {n(y - h * 0.62)}"
            f"L{n(x + w * 0.5)} {n(y - h * 0.18)}L{n(x - w * 0.5)} {n(y - h * 0.18)}"
            f"L{n(x - w * 0.2)} {n(y - h * 0.62)}L{n(x - w * 0.32)} {n(y - h * 0.62)}Z")


def broadleaf(x, y, h, seed=0) -> str:
    r = random.Random(seed)
    cx, cy, rr = x, y - h * 0.62, h * 0.38
    pts = []
    for i in range(9):
        a = 2 * math.pi * i / 9
        k = 0.85 + 0.25 * r.random()
        pts.append((cx + math.cos(a) * rr * k, cy + math.sin(a) * rr * 0.9 * k))
    return smooth(pts, True)


# ---------------------------------------------------------------- blocks
def box_faces(x0, x1, z0, z1, depth, o):
    """Front, top and right faces of an oblique box (x0..x1 wide, z0..z1 tall, depth deep)."""
    f = [ob(x0, 0, z1, o), ob(x1, 0, z1, o), ob(x1, 0, z0, o), ob(x0, 0, z0, o)]
    t = [ob(x0, 0, z1, o), ob(x0, depth, z1, o), ob(x1, depth, z1, o), ob(x1, 0, z1, o)]
    r = [ob(x1, 0, z1, o), ob(x1, depth, z1, o), ob(x1, depth, z0, o), ob(x1, 0, z0, o)]
    return f, t, r


def draw_box(art: Art, x0, x1, z0, z1, depth, o, front, top, right, outline=True, hair=False):
    f, t, r = box_faces(x0, x1, z0, z1, depth, o)
    art.path(P(f, True), fill=front)
    art.path(P(t, True), fill=top)
    art.path(P(r, True), fill=right)
    if outline:
        art.path(P(f, True) + P(t, True) + P([r[1], r[2], r[3]]), cls="h" if hair else "o")


# ---------------------------------------------------------------- globe
_COUNTRIES = None


def countries():
    global _COUNTRIES
    if _COUNTRIES is None:
        d = json.loads((REPO / "assets" / "examples" / "countries.geojson").read_text(encoding="utf-8"))
        rings = []
        for f in d["features"]:
            g = f["geometry"]
            polys = g["coordinates"] if g["type"] == "MultiPolygon" else [g["coordinates"]]
            for p in polys:
                rings.append((f["properties"].get("iso_a3"), p[0]))
        _COUNTRIES = rings
    return _COUNTRIES


_CITIES = None


def cities():
    global _CITIES
    if _CITIES is None:
        d = json.loads((REPO / "assets" / "examples" / "cities.geojson").read_text(encoding="utf-8"))
        _CITIES = [(f["geometry"]["coordinates"][0], f["geometry"]["coordinates"][1],
                    f["properties"].get("population") or 0, f["properties"].get("name"))
                   for f in d["features"]]
    return _CITIES


class Ortho:
    """Orthographic projection centred on (lon0, lat0), radius R at screen centre c."""

    def __init__(self, lon0, lat0, R, c):
        self.l0, self.p0 = math.radians(lon0), math.radians(lat0)
        self.R, self.c = R, c

    def raw(self, lon, lat):
        l, p = math.radians(lon), math.radians(lat)
        cosc = math.sin(self.p0) * math.sin(p) + math.cos(self.p0) * math.cos(p) * math.cos(l - self.l0)
        x = math.cos(p) * math.sin(l - self.l0)
        y = math.cos(self.p0) * math.sin(p) - math.sin(self.p0) * math.cos(p) * math.cos(l - self.l0)
        return x, y, cosc

    def __call__(self, lon, lat, clamp=True):
        x, y, cosc = self.raw(lon, lat)
        if cosc < 0 and clamp:
            L = math.hypot(x, y) or 1
            x, y = x / L, y / L
        return (self.c[0] + x * self.R, self.c[1] - y * self.R), cosc >= 0

    def inv(self, sx, sy):
        """Screen point (on or inside the disc) back to (lon, lat)."""
        x, y = (sx - self.c[0]) / self.R, -(sy - self.c[1]) / self.R
        rho = min(1.0, math.hypot(x, y))
        if rho < 1e-12:
            return math.degrees(self.l0), math.degrees(self.p0)
        c = math.asin(rho)
        lat = math.asin(math.cos(c) * math.sin(self.p0) + y * math.sin(c) * math.cos(self.p0) / rho)
        lon = self.l0 + math.atan2(x * math.sin(c), rho * math.cos(self.p0) * math.cos(c) - y * math.sin(self.p0) * math.sin(c))
        return math.degrees(lon), math.degrees(lat)

    def cap(self, centre, radius_deg, k=120):
        """The visible part of a spherical cap (all points within radius_deg of centre) as path data."""
        ring = small_circle(centre, radius_deg, k)
        proj = [self(lon, lat, clamp=False) for lon, lat in ring]
        vis = [v for _, v in proj]
        cx, cy, R = self.c[0], self.c[1], self.R

        def inside(lonlat):
            return ang_dist(lonlat, centre) <= radius_deg

        if all(vis):
            return P([p for p, _ in proj], True)
        if not any(vis):
            if inside((math.degrees(self.l0), math.degrees(self.p0))):
                return f"M{n(cx - R)} {n(cy)}a{n(R)} {n(R)} 0 1 0 {n(2 * R)} 0a{n(R)} {n(R)} 0 1 0 {n(-2 * R)} 0Z"
            return ""
        start = next(i for i in range(k) if vis[i] and not vis[i - 1])
        run = []
        for j in range(k):
            i = (start + j) % k
            if not vis[i]:
                break
            run.append(proj[i][0])
        a0 = math.atan2(run[-1][1] - cy, run[-1][0] - cx)
        a1 = math.atan2(run[0][1] - cy, run[0][0] - cx)
        # go round the limb from the end of the run back to its start, on the side inside the cap
        best = None
        for sgn in (1, -1):
            span = (a1 - a0) % (2 * math.pi) if sgn > 0 else -((a0 - a1) % (2 * math.pi))
            mid = a0 + span / 2
            mp = (cx + R * 0.999 * math.cos(mid), cy + R * 0.999 * math.sin(mid))
            if inside(self.inv(*mp)):
                best = span
                break
        if best is None:
            best = (a1 - a0) % (2 * math.pi)
        steps = max(2, int(abs(best) / 0.05))
        arc = [(cx + R * math.cos(a0 + best * s / steps), cy + R * math.sin(a0 + best * s / steps)) for s in range(steps + 1)]
        return P(run + arc, True)

    def land(self, simplify=0.45, only=None):
        """Land as one path (back-side points pushed to the limb)."""
        out = []
        for iso, ring in countries():
            if only and iso not in only:
                continue
            pts, vis = [], 0
            for lon, lat in ring:
                p, v = self(lon, lat)
                vis += v
                pts.append(p)
            if vis == 0:
                continue
            pts = _thin(pts, simplify)
            if len(pts) >= 3:
                out.append(P(pts, True))
        return "".join(out)

    def line(self, lonlats, curve=False):
        """A polyline on the sphere, split where it goes behind the globe."""
        segs, cur = [], []
        for lon, lat in lonlats:
            p, v = self(lon, lat, clamp=False)
            if v:
                cur.append(p)
            elif cur:
                segs.append(cur)
                cur = []
        if cur:
            segs.append(cur)
        return "".join((smooth(s) if curve and len(s) > 2 else P(s)) for s in segs if len(s) > 1)

    def graticule(self, step=30, res=6):
        d = ""
        for lon in range(-180, 180, step):
            d += self.line([(lon, lat) for lat in range(-90, 91, res)], curve=True)
        for lat in range(-90 + step, 90, step):
            d += self.line([(lon, lat) for lon in range(-180, 181, res)], curve=True)
        return d


def _thin(pts, tol):
    """Ramer-Douglas-Peucker simplification of a closed ring (screen units)."""
    if tol <= 0 or len(pts) < 4:
        return pts
    keep = [False] * len(pts)
    keep[0] = keep[-1] = True
    stack = [(0, len(pts) - 1)]
    while stack:
        i, j = stack.pop()
        (x1, y1), (x2, y2) = pts[i], pts[j]
        dx, dy = x2 - x1, y2 - y1
        L = math.hypot(dx, dy)
        best, bk = -1.0, -1
        for k in range(i + 1, j):
            x, y = pts[k]
            d = abs(dy * (x - x1) - dx * (y - y1)) / L if L > 1e-9 else math.hypot(x - x1, y - y1)
            if d > best:
                best, bk = d, k
        if best > tol:
            keep[bk] = True
            stack += [(i, bk), (bk, j)]
    return [p for p, k in zip(pts, keep) if k]


def _vec(lon, lat):
    l, p = math.radians(lon), math.radians(lat)
    return (math.cos(p) * math.cos(l), math.cos(p) * math.sin(l), math.sin(p))


def ang_dist(a, b):
    va, vb = _vec(*a), _vec(*b)
    return math.degrees(math.acos(max(-1, min(1, sum(x * y for x, y in zip(va, vb))))))


def small_circle(centre, radius_deg, k=120):
    """Points at angular distance radius_deg around centre=(lon, lat)."""
    lon0, lat0 = map(math.radians, centre)
    d = math.radians(radius_deg)
    out = []
    for i in range(k):
        b = 2 * math.pi * i / k
        lat = math.asin(math.sin(lat0) * math.cos(d) + math.cos(lat0) * math.sin(d) * math.cos(b))
        lon = lon0 + math.atan2(math.sin(b) * math.sin(d) * math.cos(lat0), math.cos(d) - math.sin(lat0) * math.sin(lat))
        out.append((math.degrees(lon), math.degrees(lat)))
    return out


def great_circle(a, b, k=48):
    """Points along the great circle from a=(lon,lat) to b."""
    l1, p1, l2, p2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    v1 = (math.cos(p1) * math.cos(l1), math.cos(p1) * math.sin(l1), math.sin(p1))
    v2 = (math.cos(p2) * math.cos(l2), math.cos(p2) * math.sin(l2), math.sin(p2))
    dot = max(-1, min(1, sum(x * y for x, y in zip(v1, v2))))
    om = math.acos(dot)
    out = []
    for i in range(k + 1):
        t = i / k
        if om < 1e-9:
            v = v1
        else:
            s1, s2 = math.sin((1 - t) * om) / math.sin(om), math.sin(t * om) / math.sin(om)
            v = tuple(s1 * x + s2 * y for x, y in zip(v1, v2))
        out.append((math.degrees(math.atan2(v[1], v[0])), math.degrees(math.asin(max(-1, min(1, v[2]))))))
    return out


def globe(art: Art, o: Ortho, ocean=None, land=None, grat=True, rim=True, shade=True, border=False):
    """Ocean disc, land, graticule, terminator-free top-left shading and rim."""
    cx, cy, R = o.c[0], o.c[1], o.R
    disc = f"M{n(cx - R)} {n(cy)}a{n(R)} {n(R)} 0 1 0 {n(2 * R)} 0a{n(R)} {n(R)} 0 1 0 {n(-2 * R)} 0Z"
    cp = art.clip(disc)
    art.path(disc, fill=ocean or art.rad([(0, PAL["water1"]), (1, PAL["water2"])], 0.38, 0.32, 0.75))
    art.group(cp=cp)
    if land is not False:
        lid = art.uid("land")
        art.defs.append(f'<path id="{lid}" d="{o.land()}"/>')
        art.body.append(f'<use href="#{lid}" ' + attrs(dict(
            fill=land or PAL["sand2"], stroke=mix(land or PAL["sand2"], PAL["ink"], 0.25) if border else "none",
            sw=0.3 if border else None, lj="round")) + "/>")
        art.land_id = lid
    if grat:
        art.path(o.graticule(), cls="h", so=0.16)
    art.end()
    return disc, cp


def globe_shade(art: Art, o: Ortho, disc: str, op=0.32):
    """Sphere shading: lit from the top left, darker limb lower right, then the rim."""
    g = art.rad([(0, "#ffffff", 0.35), (0.45, "#ffffff", 0), (0.8, PAL["ink"], op * 0.4), (1, PAL["ink"], op)],
                0.36, 0.3, 0.78)
    art.path(disc, fill=g)
    art.path(disc, cls="o")


def scallop(pts, bulge=0.42, seed=0, jitter=0.25) -> str:
    """A closed outline whose edges bulge outward as rounded lobes (cumulus, foliage)."""
    r = random.Random(seed)
    area = sum(pts[i][0] * pts[(i + 1) % len(pts)][1] - pts[(i + 1) % len(pts)][0] * pts[i][1] for i in range(len(pts)))
    sgn = 1 if area > 0 else -1       # orientation in screen space
    d = f"M{pt(pts[0])}"
    for i in range(len(pts)):
        p, q = pts[i], pts[(i + 1) % len(pts)]
        dx, dy = q[0] - p[0], q[1] - p[1]
        L = math.hypot(dx, dy)
        k = bulge * (1 - jitter + 2 * jitter * r.random())
        nx, ny = dy / L * sgn, -dx / L * sgn
        c = ((p[0] + q[0]) / 2 + nx * L * k, (p[1] + q[1]) / 2 + ny * L * k)
        d += f"Q{pt(c)} {pt(q)}"
    return d + "Z"


def ring_pts(cx, cy, rx, ry, k, a0=0.0, a1=360.0):
    return [(cx + rx * math.cos(math.radians(a0 + (a1 - a0) * i / k)), cy + ry * math.sin(math.radians(a0 + (a1 - a0) * i / k)))
            for i in range(k + 1)]


# ---------------------------------------------------------------- terrain blocks
LIGHT = (-0.55, 0.45, 0.70)   # from the upper left and slightly behind: top-left lighting on screen


def terrain(art: Art, o, Wd, D, h, nx=30, ny=12, base=14.0, cover=None, colours=None, strata=None,
            levels=6, outline=True, side=None):
    """An oblique block of terrain: a shaded surface (quads grouped by cover and shade level),
    a front and a right cut face with soil strata, and an outline.

    h(x, y) gives the height of the surface; cover(x, y) a key into colours (default 'land').
    strata is a list of (thickness, colour) from the surface down. Returns surf(x, y) -> screen.
    """
    colours = colours or {"land": PAL["sage2"]}
    xs = [Wd * i / nx for i in range(nx + 1)]
    ys = [D * j / ny for j in range(ny + 1)]
    Z = [[h(x, y) for x in xs] for y in ys]
    L = LIGHT
    Ln = math.sqrt(sum(c * c for c in L))
    groups: dict = {}
    order = []
    for j in reversed(range(ny)):
        for i in range(nx):
            z00, z10, z01, z11 = Z[j][i], Z[j][i + 1], Z[j + 1][i], Z[j + 1][i + 1]
            dzdx = ((z10 - z00) + (z11 - z01)) / 2 / (xs[1] - xs[0])
            dzdy = ((z01 - z00) + (z11 - z10)) / 2 / (ys[1] - ys[0])
            nrm = (-dzdx, -dzdy, 1.0)
            nl = math.sqrt(sum(c * c for c in nrm))
            lam = sum(a * b for a, b in zip(nrm, L)) / (nl * Ln)
            lvl = max(0, min(levels - 1, round((lam - 0.35) / 0.65 * (levels - 1))))
            key = cover((xs[i] + xs[i + 1]) / 2, (ys[j] + ys[j + 1]) / 2) if cover else "land"
            q = [ob(xs[i], ys[j], z00, o), ob(xs[i], ys[j + 1], z01, o), ob(xs[i + 1], ys[j + 1], z11, o), ob(xs[i + 1], ys[j], z10, o)]
            g = (key, lvl)
            if g not in groups:
                groups[g] = []
                order.append(g)
            groups[g].append(P(q, True))
    for g in order:
        key, lvl = g
        c = colours[key]
        shade = mix(mix(c, PAL["ink"], 0.28), mix(c, "#ffffff", 0.32), lvl / (levels - 1))
        art.path("".join(groups[g]), fill=shade, stroke=shade, sw=0.35, lj="round")
    # cut faces
    strata = strata or [(4, PAL["soil"]), (10, PAL["clay"]), (999, PAL["rock"])]
    front_top = [ob(x, 0, Z[0][i], o) for i, x in enumerate(xs)]
    right_top = [ob(Wd, y, Z[j][nx], o) for j, y in enumerate(ys)]
    for face, tops, coords, dark in (("front", Z[0], xs, 0.0), ("right", [Z[j][nx] for j in range(ny + 1)], ys, 0.18)):
        depth_acc = 0.0
        for th, col in strata:
            up = [z - depth_acc for z in tops]
            dn = [max(z - depth_acc - th, -base) for z in tops]
            if all(u_ <= -base for u_ in up):
                break
            if face == "front":
                pu = [ob(x, 0, max(u_, -base), o) for x, u_ in zip(coords, up)]
                pd = [ob(x, 0, d_, o) for x, d_ in zip(coords, dn)]
            else:
                pu = [ob(Wd, y, max(u_, -base), o) for y, u_ in zip(coords, up)]
                pd = [ob(Wd, y, d_, o) for y, d_ in zip(coords, dn)]
            art.path(P(pu + pd[::-1], True), fill=mix(col, PAL["ink"], dark))
            depth_acc += th
    if outline:
        art.path(P(front_top), cls="o")
        art.path(P(right_top), cls="o")
        art.path(P([ob(0, 0, Z[0][0], o), ob(0, 0, -base, o), ob(Wd, 0, -base, o), ob(Wd, D, -base, o), ob(Wd, D, Z[ny][nx], o)])
                 + P([ob(Wd, 0, -base, o), ob(Wd, 0, Z[0][nx], o)]), cls="o")
        back = [ob(x, D, Z[ny][i], o) for i, x in enumerate(xs)] + [ob(0, y, Z[j][0], o) for j, y in reversed(list(enumerate(ys)))]
        art.path(P([ob(x, D, Z[ny][i], o) for i, x in enumerate(xs)]) + P([ob(0, y, Z[j][0], o) for j, y in enumerate(ys)]), cls="h", so=.6)

    def surf(x, y, dz=0.0):
        return ob(x, y, h(x, y) + dz, o)
    return surf


# ---------------------------------------------------------------- buildings
def box3(a: Art, x0, x1, y0, y1, z0, z1, o, front, top, right, hair=True, sw=.35):
    """An oblique box spanning x0..x1, depth y0..y1, height z0..z1 (front, top, right faces)."""
    f = [ob(x0, y0, z1, o), ob(x1, y0, z1, o), ob(x1, y0, z0, o), ob(x0, y0, z0, o)]
    t = [ob(x0, y0, z1, o), ob(x0, y1, z1, o), ob(x1, y1, z1, o), ob(x1, y0, z1, o)]
    r = [ob(x1, y0, z1, o), ob(x1, y1, z1, o), ob(x1, y1, z0, o), ob(x1, y0, z0, o)]
    a.path(P(f, True), fill=front)
    if top:
        a.path(P(t, True), fill=top)
    a.path(P(r, True), fill=right)
    if hair:
        a.path(P(f, True) + P([t[0], t[1], t[2], t[3]]) + P([r[2], r[3]]) + P([t[2], r[2]]), stroke=PAL["ink"], sw=sw, so=.75,
               fill="none", lj="round")


def hiproof(a: Art, x0, x1, y0, y1, z, rh, o, colour, inset=None, hair=True, overhang=1.5):
    """A hipped roof over x0..x1, y0..y1 with eaves at z and the ridge rh higher, along x."""
    x0, x1, y0, y1 = x0 - overhang, x1 + overhang, y0 - overhang, y1 + overhang
    ym = (y0 + y1) / 2
    i = inset if inset is not None else (y1 - y0) / 2 * .9
    r0, r1 = ob(x0 + i, ym, z + rh, o), ob(x1 - i, ym, z + rh, o)
    e = {k: ob(x, y, z, o) for k, (x, y) in {"fl": (x0, y0), "fr": (x1, y0), "bl": (x0, y1), "br": (x1, y1)}.items()}
    light, mid, dark = mix(colour, "#ffffff", .25), colour, mix(colour, PAL["ink"], .25)
    a.path(P([e["bl"], e["br"], r1, r0], True), fill=light)
    a.path(P([e["fl"], e["bl"], r0], True), fill=light)
    a.path(P([e["fl"], e["fr"], r1, r0], True), fill=mid)
    a.path(P([e["fr"], e["br"], r1], True), fill=dark)
    if hair:
        a.path(P([e["fl"], e["fr"], e["br"]]) + P([e["fl"], r0, r1, e["fr"]]) + P([r1, e["br"]]), stroke=PAL["ink"], sw=.4, so=.8,
               fill="none", lj="round")
    # tile courses on the front slope
    lines = "".join(P([lerp2(e["fl"], r0, k / 5), lerp2(e["fr"], r1, k / 5)]) for k in range(1, 5))
    a.path(lines, stroke=PAL["ink"], sw=.25, so=.35, fill="none")


def gable(a: Art, x0, x1, y0, y1, z, rh, o, colour, hair=True, overhang=1.2):
    """A gabled roof with its ridge along the depth axis (y), gable end facing the viewer."""
    x0, x1 = x0 - overhang, x1 + overhang
    xm = (x0 + x1) / 2
    fl, fr, bl, br = ob(x0, y0, z, o), ob(x1, y0, z, o), ob(x0, y1, z, o), ob(x1, y1, z, o)
    rf, rb = ob(xm, y0, z + rh, o), ob(xm, y1, z + rh, o)
    a.path(P([fl, rf, rb, bl], True), fill=mix(colour, "#ffffff", .25))
    a.path(P([fr, rf, rb, br], True), fill=mix(colour, PAL["ink"], .22))
    if hair:
        a.path(P([bl, fl, rf, fr, br]) + P([rf, rb]), stroke=PAL["ink"], sw=.45, so=.85, fill="none", lj="round")
    return fl, fr, rf


def cylinder(a: Art, cx, cy, r, z0, z1, o, colour, hair=True, k=20):
    """An upright cylinder on the ground plane (circle of radius r at depth-scaled ellipse)."""
    def ring(z):
        return [ob(cx + r * math.cos(t), cy + r * math.sin(t), z, o) for t in [2 * math.pi * i / k for i in range(k)]]
    lo, hi = ring(z0), ring(z1)
    # silhouette: extreme left and right points of the projected ellipse
    li = min(range(k), key=lambda i: hi[i][0])
    ri = max(range(k), key=lambda i: hi[i][0])
    side = [hi[li], hi[ri], lo[ri], lo[li]]
    # front half of the lower ring
    front = [lo[(ri - j) % k] for j in range((ri - li) % k + 1)]
    g = a.lin([(0, mix(colour, "#ffffff", .3)), (.45, colour), (1, mix(colour, PAL["ink"], .3))], hi[li][0], 0, hi[ri][0], 0, user=True)
    a.path(P([hi[li], hi[ri]] + front, True), fill=g)
    a.path(P(hi, True), fill=mix(colour, "#ffffff", .2))
    if hair:
        a.path(P(hi, True) + P([hi[li], lo[li]]) + P([hi[ri], lo[ri]]) + P(front), stroke=PAL["ink"], sw=.4, so=.8, fill="none")
    return hi


def merlons(a: Art, pts, z, o, h=1.6, every=2.6, colour=None, w=1.4):
    """Battlements: small raised blocks along a polyline of (x, y) at height z."""
    colour = colour or PAL["rock1"]
    d = ""
    for (p, q) in zip(pts, pts[1:]):
        L = math.hypot(q[0] - p[0], q[1] - p[1])
        nseg = max(1, int(L / every))
        for i in range(nseg):
            t = (i + .3) / nseg
            x, y = lerp(p[0], q[0], t), lerp(p[1], q[1], t)
            b = ob(x, y, z, o)
            d += f"M{n(b[0])} {n(b[1])}v{n(-h)}h{n(w)}v{n(h)}"
    a.path(d, fill=colour, stroke=PAL["ink"], sw=.3, so=.8)
