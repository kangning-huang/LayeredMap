"""Generate the five Strata variations (strata-v1..v5) with computed isometric geometry.

Every id is prefixed per file (sv1-, sv2-, ...) so the SVGs can be inlined together.
No masks, filters or external references (CairoSVG-safe).
"""
import math, os

OUT = "/Volumes/LaCie/repos/LayeredMap/design/logo"
C = 512
A = 412
T30 = math.tan(math.radians(30))


def f(v):
    s = f"{v:.1f}"
    return s[:-2] if s.endswith(".0") else s


def pts(seq):
    return " ".join(f"{f(x)},{f(y)}" for x, y in seq)


def squircle_d(n=5.0, a=A, cx=C, cy=C, steps=36):
    out = []
    for i in range(steps * 4):
        t = 2 * math.pi * i / (steps * 4)
        ct, st = math.cos(t), math.sin(t)
        out.append((cx + a * math.copysign(abs(ct) ** (2 / n), ct),
                    cy + a * math.copysign(abs(st) ** (2 / n), st)))
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in out) + " Z"


SQ = squircle_d()


class Sheet:
    """An isometric sheet: local coords 0..100, optional in-plane rotation (deg)."""

    def __init__(self, cy, w, rot=0.0, cx=C):
        self.cx, self.cy, self.w, self.rot = cx, cy, w, rot
        self.h = w * T30

    def tf(self):
        w, h = self.w / 100, self.h / 100
        s = f"matrix({f(w*1000)/1000 if False else round(w,4)},{round(h,4)},{round(-w,4)},{round(h,4)},{f(self.cx)},{f(self.cy-self.h)})"
        if self.rot:
            s += f" rotate({self.rot} 50 50)"
        return s

    def g(self, u, v):
        if self.rot:
            a = math.radians(self.rot)
            du, dv = u - 50, v - 50
            u, v = 50 + du*math.cos(a) - dv*math.sin(a), 50 + du*math.sin(a) + dv*math.cos(a)
        return (self.cx + (u - v) * self.w / 100, self.cy - self.h + (u + v) * self.h / 100)

    def corners(self):
        return [self.g(0, 0), self.g(100, 0), self.g(100, 100), self.g(0, 100)]

    def slab(self, t, col):
        """Thickness: convex hull of the sheet and the sheet shifted down by t."""
        P = self.corners() + [(x, y + t) for x, y in self.corners()]
        return f'<polygon points="{pts(hull(P))}" fill="{col}"/>'


def hull(P):
    P = sorted(set((round(x, 2), round(y, 2)) for x, y in P))
    def cr(o, a, b): return (a[0]-o[0])*(b[1]-o[1]) - (a[1]-o[1])*(b[0]-o[0])
    lo, up = [], []
    for p in P:
        while len(lo) >= 2 and cr(lo[-2], lo[-1], p) <= 0: lo.pop()
        lo.append(p)
    for p in reversed(P):
        while len(up) >= 2 and cr(up[-2], up[-1], p) <= 0: up.pop()
        up.append(p)
    return lo[:-1] + up[:-1]


def wrap(pfx, name, defs, body, bg, rim="#0F172A", rim_op=0.08):
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <title>LayeredMap - {name}</title>
  <defs>
    <path id="{pfx}-sq" d="{SQ}"/>
    <clipPath id="{pfx}-body"><use href="#{pfx}-sq"/></clipPath>
{defs}  </defs>
  <use href="#{pfx}-sq" fill="{bg}"/>
  <g clip-path="url(#{pfx}-body)">
{body}  </g>
  <use href="#{pfx}-sq" fill="none" stroke="{rim}" stroke-opacity="{rim_op}" stroke-width="2"/>
</svg>
"""


def lg(id_, stops, x1=0, y1=0, x2=0, y2=1):
    s = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in stops)
    return f'    <linearGradient id="{id_}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}">{s}</linearGradient>\n'


def light_bg(p):
    return lg(f"{p}-bg", [(0, "#F8FAFC"), (1, "#E3E9F1")])


def viridis(p, name="rast"):
    return lg(f"{p}-{name}", [(0, "#3B528B"), (0.45, "#21918C"), (1, "#7AD151")], 0, 0, 1, 1)


# --------------------------------------------------------------- v1 BOLD
def v1_bold(size=1024):
    """Three bold sheets, exploded gap. size selects the hand-simplified 32/16 px artwork."""
    p = "sv1" if size > 32 else f"sv1x{size}"
    defs = light_bg(p) + viridis(p)
    defs += lg(f"{p}-top", [(0, "#3B82F6"), (1, "#2563EB")], 0, 0, 1, 1)
    if size <= 16:
        # 16 px: no edges, no content, larger sheets, three flat colours with clear gaps
        w, gap = 330, 150
        S = [Sheet(C - gap - 6, w), Sheet(C - 6, w), Sheet(C + gap - 6, w)]
        cols = ["#2563EB", "#FFFFFF", "#22A884"]
        body = ""
        for s, c in zip(reversed(S), reversed(cols)):
            body += f'    <polygon points="{pts(s.corners())}" fill="{c}"/>\n'
        # crisp outline on white sheet so it separates from the light container
        mid = S[1]
        body = body.replace(f'<polygon points="{pts(mid.corners())}" fill="#FFFFFF"/>',
                            f'<polygon points="{pts(mid.corners())}" fill="#FFFFFF" stroke="#94A3B8" stroke-width="22" stroke-linejoin="round"/>')
        return wrap(p, "Strata Bold (16 px)", defs, body, f"url(#{p}-bg)")
    w, gap, t = 300, 138, 24
    if size <= 32:
        t = 30
    S = [Sheet(C - gap - 10, w), Sheet(C - 10, w), Sheet(C + gap - 10, w)]
    body = "    <!-- bottom: raster -->\n"
    s = S[2]
    body += f'    {s.slab(t, "#1A6B6E")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-rast)"/></g>\n'
    s = S[1]
    body += "    <!-- middle: vectors -->\n"
    body += f'    {s.slab(t, "#B3C0D0")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="#FFFFFF"/>'
    if size > 32:
        body += '<polyline points="0,58 46,52 100,64" fill="none" stroke="#C3CEDC" stroke-width="3.2" stroke-linejoin="round"/><polyline points="46,52 56,100" fill="none" stroke="#C3CEDC" stroke-width="3.2"/>'
    body += "</g>\n"
    s = S[0]
    body += "    <!-- top: points -->\n"
    body += f'    {s.slab(t, "#1D4ED8")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-top)"/>'
    r = 12 if size > 32 else 15
    body += f'<circle cx="50" cy="50" r="{r}" fill="#FFFFFF"/>'
    body += "</g>\n"
    return wrap(p, "Strata Bold", defs, body, f"url(#{p}-bg)")


# --------------------------------------------------------------- v2 LINK
def v2_link():
    p = "sv2"
    defs = light_bg(p) + viridis(p)
    defs += lg(f"{p}-top", [(0, "#3B82F6"), (1, "#2563EB")], 0, 0, 1, 1)
    w, gap, t = 262, 132, 16
    S = [Sheet(C - gap - 4, w), Sheet(C - 4, w), Sheet(C + gap - 4, w)]
    u, v = 62, 60   # the shared location (local coords)
    P = [s.g(u, v) for s in S]
    line = lambda a, b: f'<line x1="{f(a[0])}" y1="{f(a[1])}" x2="{f(b[0])}" y2="{f(b[1])}" stroke="#0F172A" stroke-opacity="0.55" stroke-width="5" stroke-linecap="round"/>'
    body = "    <!-- bottom: raster -->\n"
    s = S[2]
    body += f'    {s.slab(t, "#1F6F78")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-rast)"/>'
    body += f'<ellipse cx="{u}" cy="{v}" rx="9" ry="9" fill="none" stroke="#FFFFFF" stroke-width="2.6"/><circle cx="{u}" cy="{v}" r="3" fill="#FFFFFF"/></g>\n'
    s = S[1]
    body += "    <!-- middle: polygons -->\n"
    body += f'    {s.slab(t, "#B8C4D3")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="#FFFFFF"/>'
    body += '<polygon points="44,40 80,34 86,76 50,82" fill="#D6E3FD"/>'
    body += '<g fill="none" stroke="#9AABC2" stroke-width="1.5" stroke-linejoin="round"><polygon points="44,40 80,34 86,76 50,82"/><polyline points="0,44 44,40"/><polyline points="80,34 100,30"/><polyline points="50,82 46,100"/><polyline points="86,76 100,80"/><polyline points="44,40 40,0"/></g>'
    body += f'<circle cx="{u}" cy="{v}" r="9" fill="none" stroke="#2563EB" stroke-width="2.6"/><circle cx="{u}" cy="{v}" r="3" fill="#2563EB"/></g>\n'
    s = S[0]
    body += "    <!-- top: points; the linked point -->\n"
    body += f'    {s.slab(t, "#1D4ED8")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-top)" fill-opacity="0.94"/>'
    body += '<g fill="#FFFFFF" fill-opacity="0.55"><circle cx="24" cy="30" r="4.5"/><circle cx="30" cy="70" r="4.5"/><circle cx="76" cy="22" r="4.5"/></g>'
    body += '</g>\n'
    a, b = P[0], P[2]
    body += "    <!-- drop-line: the same location through every layer -->\n"
    body += f'    <line x1="{f(a[0])}" y1="{f(a[1])}" x2="{f(b[0])}" y2="{f(b[1])}" stroke="#FFFFFF" stroke-width="16" stroke-linecap="round"/>\n'
    body += f'    <line x1="{f(a[0])}" y1="{f(a[1])}" x2="{f(b[0])}" y2="{f(b[1])}" stroke="#1E3A8A" stroke-width="6" stroke-linecap="round"/>\n'
    for q, col in ((P[1], "#2563EB"), (P[2], "#FFFFFF")):
        body += f'    <circle cx="{f(q[0])}" cy="{f(q[1])}" r="11" fill="{col}" stroke="#1E3A8A" stroke-width="5"/>\n'
    body += f'    <circle cx="{f(a[0])}" cy="{f(a[1])}" r="24" fill="#FFFFFF" stroke="#1E3A8A" stroke-width="6"/>\n'
    return wrap(p, "Strata Link", defs, body, f"url(#{p}-bg)")


# --------------------------------------------------------------- v3 BLUE
def v3_blue():
    p = "sv3"
    defs = lg(f"{p}-bg", [(0, "#3B82F6"), (1, "#1E4FD6")])
    w, gap, t = 280, 130, 14
    S = [Sheet(C - gap - 4, w), Sheet(C - 4, w), Sheet(C + gap - 4, w)]
    body = ""
    s = S[2]
    body += "    <!-- bottom: raster, as soft contours -->\n"
    body += f'    {s.slab(t, "#0B2A7A").replace("/>", " fill-opacity=\"0.35\"/>")}\n'
    body += f'    <g transform="{s.tf()}"><rect width="100" height="100" fill="#FFFFFF" fill-opacity="0.42"/>'
    body += '<g fill="none" stroke="#FFFFFF" stroke-opacity="0.55" stroke-width="2"><ellipse cx="70" cy="70" rx="24" ry="20"/><ellipse cx="72" cy="72" rx="12" ry="10"/></g></g>\n'
    s = S[1]
    body += "    <!-- middle: polygons -->\n"
    body += f'    {s.slab(t, "#0B2A7A").replace("/>", " fill-opacity=\"0.35\"/>")}\n'
    body += f'    <g transform="{s.tf()}"><rect width="100" height="100" fill="#FFFFFF" fill-opacity="0.72"/>'
    body += '<g fill="none" stroke="#2563EB" stroke-opacity="0.45" stroke-width="1.8" stroke-linejoin="round"><polyline points="0,46 32,42 58,60 100,54"/><polyline points="58,60 66,100"/><polyline points="32,42 36,0"/></g></g>\n'
    s = S[0]
    body += "    <!-- top: points -->\n"
    body += f'    {s.slab(t, "#DCE6FB")}\n'
    body += f'    <g transform="{s.tf()}"><rect width="100" height="100" fill="#FFFFFF"/>'
    body += '<g fill="#2563EB"><circle cx="30" cy="36" r="6"/><circle cx="68" cy="28" r="6"/><circle cx="58" cy="68" r="6"/></g>'
    body += '<circle cx="58" cy="68" r="11.5" fill="none" stroke="#2563EB" stroke-opacity="0.35" stroke-width="2.4"/></g>\n'
    return wrap(p, "Strata Blue", defs, body, f"url(#{p}-bg)", rim="#0B2A7A", rim_op=0.25)


# --------------------------------------------------------------- v4 CUTOUT
def v4_cutout():
    p = "sv4"
    defs = light_bg(p) + viridis(p)
    defs += lg(f"{p}-top", [(0, "#3B82F6"), (1, "#2563EB")], 0, 0, 1, 1)
    defs += f'    <clipPath id="{p}-fp"><rect width="100" height="100"/></clipPath>\n'
    w, gap, t = 280, 122, 18
    S = [Sheet(C - gap - 6, w), Sheet(C - 6, w), Sheet(C + gap - 6, w)]
    body = ""
    s = S[2]
    body += "    <!-- bottom: raster, full footprint -->\n"
    body += f'    {s.slab(t, "#1F6F78")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-rast)"/></g>\n'
    s = S[1]
    body += "    <!-- middle: road network only, cut to the footprint -->\n"
    roads = '<polyline points="0,32 100,40"/><polyline points="0,72 100,66"/><polyline points="32,0 36,100"/><polyline points="72,0 66,100"/><path d="M 0 100 C 30 60, 60 50, 100 4"/>'
    body += f'    <g transform="{s.tf()}">'
    body += '<rect width="100" height="100" fill="#FFFFFF" fill-opacity="0.45" stroke="#FFFFFF" stroke-width="1.2"/>'
    body += f'<g fill="none" stroke-linejoin="round" clip-path="url(#{p}-fp)"><g stroke="#475569" stroke-opacity="0.45" stroke-width="7">{roads}</g><g stroke="#FFFFFF" stroke-width="4.4">{roads}</g></g></g>\n'
    s = S[0]
    body += "    <!-- top: one district outline, cut to the footprint -->\n"
    dist = [(10, 22), (48, 6), (90, 16), (96, 58), (74, 94), (30, 92), (6, 62)]
    poly = [s.g(*q) for q in dist]
    body += f'    <polygon points="{pts(hull(poly + [(x, y + 16) for x, y in poly]))}" fill="#1D4ED8" stroke="#1D4ED8" stroke-width="10" stroke-linejoin="round"/>\n'
    body += f'    <g transform="{s.tf()}"><polygon points="{pts(dist)}" fill="url(#{p}-top)" stroke="#3B82F6" stroke-width="3.6" stroke-linejoin="round"/>'
    body += '<circle cx="56" cy="54" r="7" fill="#FFFFFF"/><circle cx="56" cy="54" r="13" fill="none" stroke="#FFFFFF" stroke-opacity="0.45" stroke-width="2.4"/></g>\n'
    return wrap(p, "Strata Cutout", defs, body, f"url(#{p}-bg)")


# --------------------------------------------------------------- v5 NIGHT FAN
def v5_night():
    p = "sv5"
    defs = lg(f"{p}-bg", [(0, "#1B2640"), (1, "#0A101E")])
    defs += viridis(p)
    defs += lg(f"{p}-top", [(0, "#4F8DF7"), (1, "#2563EB")], 0, 0, 1, 1)
    w, gap, t = 262, 118, 14
    S = [Sheet(C - gap + 4, w, rot=0), Sheet(C + 4, w, rot=-9), Sheet(C + gap + 4, w, rot=-18)]
    body = "    <!-- faint stars / graticule dots -->\n    <g fill=\"#FFFFFF\" fill-opacity=\"0.18\">"
    for x, y in [(210, 220), (820, 260), (760, 180), (250, 820), (840, 780), (180, 520)]:
        body += f'<circle cx="{x}" cy="{y}" r="5"/>'
    body += "</g>\n"
    s = S[2]
    body += "    <!-- bottom: raster (fanned -18 deg) -->\n"
    body += f'    {s.slab(t, "#123F4A")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-rast)"/></g>\n'
    s = S[1]
    body += "    <!-- middle: polygons (fanned -9 deg) -->\n"
    body += f'    {s.slab(t, "#0F172A")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="#26344F"/>'
    body += '<g fill="none" stroke="#7DD3FC" stroke-opacity="0.75" stroke-width="1.6" stroke-linejoin="round"><polyline points="0,46 32,42 58,60 100,54"/><polyline points="58,60 66,100"/><polyline points="32,42 36,0"/><polyline points="0,74 30,70 58,60"/></g></g>\n'
    s = S[0]
    body += "    <!-- top: points -->\n"
    body += f'    {s.slab(t, "#1A3FB0")}\n    <g transform="{s.tf()}"><rect width="100" height="100" fill="url(#{p}-top)"/>'
    body += '<g fill="#FFFFFF"><circle cx="30" cy="36" r="6"/><circle cx="68" cy="28" r="6"/><circle cx="58" cy="68" r="6"/></g>'
    body += '<circle cx="58" cy="68" r="11.5" fill="none" stroke="#FFFFFF" stroke-opacity="0.45" stroke-width="2.4"/></g>\n'
    return wrap(p, "Strata Night", defs, body, f"url(#{p}-bg)", rim="#FFFFFF", rim_op=0.14)


VARIANTS = [
    ("1", "bold", v1_bold),
    ("2", "link", v2_link),
    ("3", "blue", v3_blue),
    ("4", "cutout", v4_cutout),
    ("5", "night", v5_night),
]

if __name__ == "__main__":
    for n, name, fn in VARIANTS:
        path = os.path.join(OUT, f"strata-v{n}-{name}.svg")
        open(path, "w").write(fn())
        print(path)
    # hand-simplified small-size artwork for v1
    open(os.path.join(OUT, "strata-v1-bold-32.svg"), "w").write(v1_bold(32))
    open(os.path.join(OUT, "strata-v1-bold-16.svg"), "w").write(v1_bold(16))
