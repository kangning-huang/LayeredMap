"""Generate NimbleMap app-icon concept SVGs.

Every concept is a function concept(size, dark) -> (title, defs, body, bg) where
size is "L" (1024 master), "32" or "16" (hand-simplified small artwork drawn on the
same 1024 canvas, rendered at 32/16 px), and dark selects the dark appearance.
A tinted (mono) variant is derived from the dark one by luminance.

Pipeline rules carried over from design/logo: 1024 canvas, 824 px squircle
(superellipse n = 5), no masks or filters (CairoSVG ignores them), ids prefixed
per file so many SVGs can be inlined into one page.

Usage:  mamba run -n lm-logo python src/gen.py
"""
import math, os, re

D = "/Volumes/LaCie/repos/LayeredMap/design/logo-nimble"
C = 512
A = 412  # squircle half-size -> 824 px body

CORAL = "#FF5A3C"
BLUE = "#2563EB"


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


def lg(id_, stops, x1=0, y1=0, x2=0, y2=1):
    s = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in stops)
    return f'    <linearGradient id="{id_}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}">{s}</linearGradient>\n'


def pin_d(cx, cy, R, L):
    """Upright location pin: head circle (cx, cy, R), tip at (cx, cy + L)."""
    phi = math.acos(R / L)
    rx, ry = cx + R * math.sin(phi), cy + R * math.cos(phi)
    lx = cx - R * math.sin(phi)
    return f"M{f(cx)} {f(cy + L)} L{f(rx)} {f(ry)} A{f(R)} {f(R)} 0 1 0 {f(lx)} {f(ry)} Z"


def bez(p0, p1, p2, p3, t):
    u = 1 - t
    return tuple(u**3 * a + 3 * u * u * t * b + 3 * u * t * t * c + t**3 * d for a, b, c, d in zip(p0, p1, p2, p3))


def bez_d(p0, p1, p2, p3, t):
    u = 1 - t
    return tuple(3 * u * u * (b - a) + 6 * u * t * (c - b) + 3 * t * t * (d - c) for a, b, c, d in zip(p0, p1, p2, p3))


def mtx(o, ex, ey):
    """Affine matrix mapping the unit square 0..100 onto origin o with edge vectors ex, ey."""
    return f"matrix({f(ex[0]/100)},{f(ex[1]/100)},{f(ey[0]/100)},{f(ey[1]/100)},{f(o[0])},{f(o[1])})"


def mtx4(o, ex, ey):
    return f"matrix({ex[0]/100:.4f},{ex[1]/100:.4f},{ey[0]/100:.4f},{ey[1]/100:.4f},{f(o[0])},{f(o[1])})"


# --------------------------------------------------------------------- 1 DART
def dart(size, dk):
    sky = lg("bg", [(0, "#16233B"), (1, "#0E1729")] if dk else [(0, "#EAF4FF"), (1, "#D5E7FC")])
    land, coast = ("#1F3A35", "#2F5E57") if dk else ("#D3EEDB", "#A9D8BD")
    defs = sky
    sw = {"L": 1, "32": 1.7, "16": 2.6}[size]
    body = "    <!-- the map below: a coast -->\n"
    body += f'    <path d="M100 676 C 250 640, 330 720, 470 700 S 700 610, 924 640 V 924 H 100 Z" fill="{land}"/>\n'
    if size == "L":
        body += f'    <path d="M100 676 C 250 640, 330 720, 470 700 S 700 610, 924 640" fill="none" stroke="{coast}" stroke-width="6"/>\n'
        body += f'    <path d="M560 924 C 590 830, 660 780, 760 764 S 880 740, 924 742" fill="none" stroke="{"#2A4B45" if dk else "#FFFFFF"}" stroke-width="14" stroke-linecap="round"/>\n'
    # dotted route from a start point into the plane's tail
    if size == "16":
        body += f'    <path d="M262 770 C 300 690, 360 640, 432 604" fill="none" stroke="{CORAL}" stroke-width="54" stroke-linecap="round"/>\n'
    else:
        dash = "0 46" if size == "L" else "0 70"
        body += f'    <path d="M250 784 C 296 700, 350 650, 438 606" fill="none" stroke="{CORAL}" stroke-width="{f(22*sw)}" stroke-linecap="round" stroke-dasharray="{dash}"/>\n'
        body += f'    <circle cx="250" cy="784" r="{f(26*sw)}" fill="{"#0E1729" if dk else "#FFFFFF"}" stroke="{CORAL}" stroke-width="{f(12*sw)}"/>\n'
    # paper plane, nose up-right; facets: upper wing, keel, inner fold
    N, B, Cc, Dd, F = (806, 300), (226, 446), (466, 548), (556, 716), (500, 606)
    if size != "L":
        N, B, Cc, Dd, F = (830, 280), (200, 446), (460, 556), (566, 744), (502, 620)
    body += f'    <polygon points="{pts([N, Cc, Dd])}" fill="{"#8FB0E8" if dk else "#8DB4F2"}"/>\n'
    body += f'    <polygon points="{pts([Cc, F, Dd])}" fill="{"#5E86CF" if dk else "#4F86E8"}"/>\n'
    defs += f'    <clipPath id="wing"><polygon points="{pts([N, B, Cc])}"/></clipPath>\n'
    body += f'    <polygon points="{pts([N, B, Cc])}" fill="#FFFFFF"/>\n'
    if size == "L":
        body += '    <g clip-path="url(#wing)">\n'
        body += '      <path d="M300 520 C 420 470, 470 360, 600 380 S 760 330, 820 300" fill="none" stroke="#7DB4F7" stroke-width="20" stroke-linecap="round"/>\n'
        body += '      <path d="M520 420 L 640 470 L 560 540 Z" fill="#CDEBD7"/>\n'
        body += '      <path d="M330 420 L 700 470" fill="none" stroke="#E2E8F0" stroke-width="10"/>\n'
        body += '    </g>\n'
    elif size == "32":
        body += '    <path d="M330 500 C 440 460, 500 380, 640 370" fill="none" stroke="#7DB4F7" stroke-width="34" stroke-linecap="round" clip-path="url(#wing)"/>\n'
    return "Dart", defs, body, "url(#bg)"


# ------------------------------------------------------------------- 2 FEATHER
def feather(size, dk):
    defs = lg("bg", [(0, "#1B2434"), (1, "#111827")] if dk else [(0, "#FBF7EF"), (1, "#F0E7D6")])
    sea, land = ("#3B7FE6", "#3DAE78") if dk else ("#3E86F0", "#47B97F")
    sea_l, land_d = ("#7FB0F5", "#2B8C5E") if dk else ("#9CC3FA", "#2F9566")
    P0, P1, P2, P3 = (312, 752), (420, 600), (560, 420), (742, 262)   # rachis (quill end -> tip)
    big = size != "L"
    if big:
        P0, P1, P2, P3 = (300, 770), (410, 610), (560, 420), (760, 244)
    WL, WR = (150, 120) if not big else (176, 150)
    notch = size == "L"
    left, right = [], []
    n = 60
    for i in range(n + 1):
        t = i / n
        x, y = bez(P0, P1, P2, P3, t)
        dx, dy = bez_d(P0, P1, P2, P3, t)
        L = math.hypot(dx, dy); nx, ny = -dy / L, dx / L  # left normal
        prof = math.sin(math.pi * min(1, t * 1.08)) ** 0.75 if t < 0.925 else math.sin(math.pi * t) ** 0.75
        prof = max(0.0, prof) * (0.35 + 0.65 * t ** 0.35)
        back = 34 * prof  # barbs sweep back toward the quill
        left.append((x + nx * WL * prof - dx / L * back, y + ny * WL * prof - dy / L * back))
        right.append((x - nx * WR * prof - dx / L * back, y - ny * WR * prof - dy / L * back))
    left[0] = right[0] = P0
    rach = [bez(P0, P1, P2, P3, i / n) for i in range(n + 1)]
    # vanes as polygons between rachis and edge
    lv = rach + left[::-1]
    if notch:  # split in the right vane at t ~ 0.52
        k = int(0.52 * n)
        r2 = right[:]
        x, y = rach[k]
        r2[k] = (x + (right[k][0] - x) * 0.18, y + (right[k][1] - y) * 0.18)
        r2[k + 1] = (x + (right[k + 1][0] - x) * 0.55, y + (right[k + 1][1] - y) * 0.55)
        rv = rach + r2[::-1]
    else:
        rv = rach + right[::-1]
    defs += f'    <clipPath id="lv"><polygon points="{pts(lv)}"/></clipPath>\n'
    defs += f'    <clipPath id="rv"><polygon points="{pts(rv)}"/></clipPath>\n'
    body = "    <!-- feather: sea vane | coast rachis | land vane -->\n"
    body += f'    <polygon points="{pts(lv)}" fill="{sea}"/>\n'
    body += f'    <polygon points="{pts(rv)}" fill="{land}"/>\n'
    if size == "L":
        # barbs as contour-like strokes
        bl, br = "", ""
        for i in range(6, n - 2, 5):
            t = i / n
            x, y = rach[i]
            dx, dy = bez_d(P0, P1, P2, P3, t); L = math.hypot(dx, dy)
            ux, uy = dx / L, dy / L; nx, ny = -uy, ux
            bl += f'<path d="M{f(x)} {f(y)} q {f(nx*80 - ux*20)} {f(ny*80 - uy*20)} {f(nx*170 - ux*70)} {f(ny*170 - uy*70)}"/>'
            br += f'<path d="M{f(x)} {f(y)} q {f(-nx*70 - ux*20)} {f(-ny*70 - uy*20)} {f(-nx*150 - ux*70)} {f(-ny*150 - uy*70)}"/>'
        body += f'    <g clip-path="url(#lv)" fill="none" stroke="{sea_l}" stroke-width="7" stroke-linecap="round" stroke-dasharray="26 18">{bl}</g>\n'
        body += f'    <g clip-path="url(#rv)" fill="none" stroke="{land_d}" stroke-width="7" stroke-linecap="round">{br}</g>\n'
    # rachis = coastline / road, and the quill
    rw = {"L": 16, "32": 26, "16": 40}[size]
    body += f'    <path d="M{f(P0[0])} {f(P0[1])} C {f(P1[0])} {f(P1[1])}, {f(P2[0])} {f(P2[1])}, {f(P3[0])} {f(P3[1])}" fill="none" stroke="#FFFFFF" stroke-width="{rw}" stroke-linecap="round"/>\n'
    qx, qy = P0[0] - 0.42 * (P1[0] - P0[0]), P0[1] - 0.42 * (P1[1] - P0[1])
    body += f'    <path d="M{f(P0[0])} {f(P0[1])} L {f(qx)} {f(qy)}" stroke="{"#E5E7EB" if dk else "#8A7F6E"}" stroke-width="{rw}" stroke-linecap="round"/>\n'
    # the place
    if size != "16":
        px, py = bez(P0, P1, P2, P3, 0.56)
        dx, dy = bez_d(P0, P1, P2, P3, 0.56); L = math.hypot(dx, dy)
        px, py = px + dy / L * 66, py - dx / L * 66
        r = 30 if size == "L" else 46
        body += f'    <circle cx="{f(px)}" cy="{f(py)}" r="{r}" fill="{CORAL}" stroke="#FFFFFF" stroke-width="{r*0.42:.0f}"/>\n'
    return "Feather", defs, body, "url(#bg)"


# ---------------------------------------------------------------------- 3 LIFT
def lift(size, dk):
    defs = lg("bg", [(0, "#152238"), (1, "#0C1424")] if dk else [(0, "#EDF4FF"), (1, "#D7E5F9")])
    w = 300 if size == "L" else 330
    h = w * math.tan(math.radians(30))
    cy = 452
    # in-plane tilt of the sheet: rotate the iso axes a little
    a = math.radians(-9)
    ex = (w * math.cos(a) + 0, w * math.sin(a) * 0 + h)  # placeholder, replaced below
    # iso axes: u -> (w, h), v -> (-w, h); tilt = rotate both in the screen plane
    def rot(v):
        return (v[0] * math.cos(a) - v[1] * math.sin(a), v[0] * math.sin(a) + v[1] * math.cos(a))
    u, v = rot((w, h)), rot((-w, h))
    top = (C - (u[0] + v[0]) / 2, cy - (u[1] + v[1]) / 2)
    corners = [top, (top[0] + u[0], top[1] + u[1]), (top[0] + u[0] + v[0], top[1] + u[1] + v[1]), (top[0] + v[0], top[1] + v[1])]
    body = ""
    # shadow on the ground: same footprint, flattened and smaller, dropped well below = "it lifted off"
    s = 0.78
    sc = (C + 22, 742)
    sh = [(sc[0] + (x - C) * s, sc[1] + (y - cy) * s * 0.62) for x, y in corners]
    body += f'    <polygon points="{pts(sh)}" fill="{"#000000" if dk else "#1E3A8A"}" fill-opacity="{0.35 if dk else (0.13 if size == "L" else 0.22)}" stroke-linejoin="round"/>\n'
    # sheet thickness on the two front edges
    t = 16 if size == "L" else 24
    L_, Bm, R_ = corners[3], corners[2], corners[1]
    body += f'    <polygon points="{pts([L_, Bm, R_, (R_[0], R_[1] + t), (Bm[0], Bm[1] + t), (L_[0], L_[1] + t)])}" fill="{"#7A93B8" if dk else "#AFC3DD"}"/>\n'
    M = mtx4(top, u, v)
    if size == "L":
        content = f'''      <rect width="100" height="100" fill="#FFFFFF"/>
      <path d="M0 64 C 20 58, 30 74, 52 70 S 80 54, 100 58 V 100 H 0 Z" fill="#BFDBFE"/>
      <path d="M58 6 L 88 12 L 84 40 L 60 36 Z" fill="#CDEEDA"/>
      <path d="M0 30 L 100 40" stroke="#E2E8F0" stroke-width="3.2" fill="none"/>
      <path d="M34 0 L 42 62" stroke="#E2E8F0" stroke-width="3.2" fill="none"/>
      <path d="M8 88 C 30 80, 46 46, 70 24" stroke="{BLUE}" stroke-width="2.6" stroke-dasharray="0.1 5" stroke-linecap="round" fill="none"/>'''
    elif size == "32":
        content = '      <rect width="100" height="100" fill="#FFFFFF"/>\n      <path d="M0 62 C 24 56, 40 74, 100 56 V 100 H 0 Z" fill="#60A5FA"/>'
    else:
        content = '      <rect width="100" height="100" fill="#FFFFFF"/>\n      <path d="M0 58 L 100 50 V 100 H 0 Z" fill="#3B82F6"/>'
    body += f'    <g transform="{M}">\n{content}\n    </g>\n'
    # a pin standing on the lifting sheet
    ux, uy = top[0] + u[0] * 0.70 + v[0] * 0.24, top[1] + u[1] * 0.70 + v[1] * 0.24
    R, Lp = (54, 128) if size == "L" else (70, 160)
    body += f'    <ellipse cx="{f(ux)}" cy="{f(uy)}" rx="{R*0.55:.0f}" ry="{R*0.3:.0f}" fill="#0F172A" fill-opacity="0.18"/>\n'
    body += f'    <path d="{pin_d(ux, uy - Lp, R, Lp)}" fill="{CORAL}"/>\n'
    body += f'    <circle cx="{f(ux)}" cy="{f(uy - Lp)}" r="{R*0.42:.0f}" fill="#FFFFFF"/>\n'
    return "Lift", defs, body, "url(#bg)"


# --------------------------------------------------------------------- 4 HOVER
BIRD = {
    # local frame: head at (0,0), beak to lower-left, tail lower-right
    "farwing": "M70 70 C 180 20, 300 -60, 392 -98 C 336 -10, 236 84, 132 140 Z",
    "wing": "M44 64 C 104 -40, 196 -160, 300 -236 C 286 -118, 210 10, 112 118 Z",
    "tail": "M110 214 L 236 330 L 190 318 L 184 392 L 136 256 Z",
    "beak": "M-34 10 L -214 148 L -22 36 Z",
}


def hover(size, dk):
    defs = lg("bg", [(0, "#14213A"), (1, "#0D1628")] if dk else [(0, "#F4F8F3"), (1, "#E4EFE3")])
    river = "#2B5C8F" if dk else "#BFDBFE"
    block = "#1C2D47" if dk else "#FFFFFF"
    body = "    <!-- quiet map: river and street blocks -->\n"
    rw = {"L": 64, "32": 90, "16": 120}[size]
    body += f'    <path d="M100 330 C 260 380, 300 560, 420 640 S 700 760, 924 720" fill="none" stroke="{river}" stroke-width="{rw}" stroke-linecap="round"/>\n'
    if size == "L":
        body += f'    <g fill="{block}"><rect x="600" y="170" width="120" height="86" rx="10"/><rect x="740" y="170" width="90" height="86" rx="10"/><rect x="600" y="276" width="120" height="70" rx="10"/><rect x="170" y="760" width="110" height="80" rx="10"/><rect x="300" y="790" width="90" height="70" rx="10"/></g>\n'
    # bird
    s = {"L": 0.98, "32": 1.08, "16": 1.12}[size]
    tx, ty = (452, 452) if size == "L" else (440, 446)
    tip = (tx + -214 * s, ty + 148 * s)
    tgt = (tip[0] - 26, tip[1] + 22)
    r = {"L": 30, "32": 46, "16": 0}[size]
    if size == "L":
        body += f'    <circle cx="{f(tgt[0])}" cy="{f(tgt[1])}" r="62" fill="none" stroke="{CORAL}" stroke-opacity="0.35" stroke-width="8"/>\n'
    if r:
        body += f'    <circle cx="{f(tgt[0])}" cy="{f(tgt[1])}" r="{r}" fill="{CORAL}" stroke="{"#0D1628" if dk else "#FFFFFF"}" stroke-width="{r*0.4:.0f}"/>\n'
    deep, mid, light = ("#60A5FA", "#3B82F6", "#1E40AF") if dk else ("#1E40AF", "#3B82F6", "#93C5FD")
    if dk:
        deep, mid, light = "#93C5FD", "#3B82F6", "#3355A8"
    body += f'    <g transform="translate({tx} {ty}) scale({s})">\n'
    body += f'      <path d="{BIRD["farwing"]}" fill="{light}"/>\n'
    body += f'      <path d="{BIRD["tail"]}" fill="{deep}"/>\n'
    body += f'      <ellipse cx="75" cy="115" rx="{58 if size == "L" else 66}" ry="148" transform="rotate(-33 75 115)" fill="{deep}"/>\n'
    body += f'      <circle cx="0" cy="0" r="{48 if size == "L" else 56}" fill="{deep}"/>\n'
    beak = BIRD["beak"] if size == "L" else "M-38 6 L -214 148 L -16 42 Z"
    body += f'      <path d="{beak}" fill="{deep}"/>\n'
    body += f'      <path d="{BIRD["wing"]}" fill="{mid}"/>\n'
    if size == "L":
        body += f'      <circle cx="-6" cy="-10" r="9" fill="{"#0D1628" if dk else "#FFFFFF"}"/>\n'
    body += '    </g>\n'
    return "Hover", defs, body, "url(#bg)"


# ----------------------------------------------------------------- 5 QUICK PIN
def hop(size, dk):
    defs = lg("bg", [(0, "#16233A"), (1, "#0E1729")] if dk else [(0, "#F3F6F9"), (1, "#E4EAF0")])
    road = "#24344F" if dk else "#FFFFFF"
    water, park = ("#1E4470", "#1E3B33") if dk else ("#CFE3F8", "#D6EEDC")
    body = ""
    if size == "L":
        body += f'    <path d="M640 100 C 690 250, 820 300, 924 300 V 100 Z" fill="{water}"/>\n'
        body += f'    <rect x="150" y="160" width="190" height="150" rx="18" fill="{park}"/>\n'
        body += f'    <g stroke="{road}" stroke-width="26" stroke-linecap="round" fill="none"><path d="M100 400 L 924 470"/><path d="M420 100 L 360 924"/><path d="M100 760 L 924 700"/></g>\n'
    else:
        body += f'    <path d="M600 100 C 660 260, 820 320, 924 320 V 100 Z" fill="{water}"/>\n'
    # trajectory: dotted arc from the last place to the landing spot
    start, land = (220, 744), (604, 772)
    sw = {"L": 22, "32": 34, "16": 50}[size]
    if size == "16":
        body += f'    <path d="M{start[0]} {start[1]} C 236 470, 400 330, 540 360" fill="none" stroke="{BLUE}" stroke-width="{sw}" stroke-linecap="round"/>\n'
    else:
        body += f'    <path d="M{start[0]} {start[1]} C 226 470, 390 320, 548 352" fill="none" stroke="{BLUE}" stroke-width="{sw}" stroke-linecap="round" stroke-dasharray="0 {sw*2.2:.0f}"/>\n'
        body += f'    <circle cx="{start[0]}" cy="{start[1]}" r="{sw*1.15:.0f}" fill="{BLUE}"/>\n'
    # landing ring (where the tip is about to touch)
    lw = {"L": 14, "32": 22, "16": 30}[size]
    body += f'    <ellipse cx="{land[0]}" cy="{land[1]}" rx="{96 if size == "L" else 110}" ry="{34 if size == "L" else 40}" fill="none" stroke="{BLUE}" stroke-width="{lw}"/>\n'
    # pin, leaning into the hop (rotated about its tip)
    R, L = (128, 300) if size == "L" else (150, 340)
    tipx, tipy = land[0] - 18, land[1] - 26
    body += f'    <g transform="rotate(16 {tipx} {tipy})">\n'
    body += f'      <path d="{pin_d(tipx, tipy - L, R, L)}" fill="{CORAL}"/>\n'
    body += f'      <circle cx="{tipx}" cy="{tipy - L}" r="{R*0.42:.0f}" fill="#FFFFFF"/>\n'
    body += '    </g>\n'
    return "Hop", defs, body, "url(#bg)"


# --------------------------------------------------------------------- 6 WINGS
def wings(size, dk):
    defs = lg("bg", [(0, "#13254F"), (1, "#0A1733")] if dk else [(0, "#3B82F6"), (1, "#1D4ED8")], 0, 0, 0.4, 1)
    big = size != "L"
    hx = C
    ht, hb = (476, 736) if not big else (470, 764)        # hinge top/bottom: wide, shallow panels
    ox_l, ox_r = (214, 810) if not big else (186, 838)
    rise = 112 if not big else 124                        # outer edges sit higher: a V, wings up
    Lo = (ox_l, ht - rise); Ro = (ox_r, ht - rise)
    H = hb - ht
    exL = (hx - ox_l, rise); ey = (0, H)
    exR = (ox_r - hx, -rise)
    body = ""
    # left panel: sea and coast; right panel: land and streets. One route crosses the fold.
    if size == "L":
        lc = f'''      <rect width="100" height="100" fill="#FFFFFF"/>
      <path d="M0 0 H 46 C 40 22, 64 34, 58 54 S 74 84, 66 100 H 0 Z" fill="#BFDBFE"/>
      <path d="M46 0 C 40 22, 64 34, 58 54 S 74 84, 66 100" fill="none" stroke="#93C5FD" stroke-width="1.6"/>'''
        rc = f'''      <rect width="100" height="100" fill="#EEF4FF"/>
      <rect x="44" y="12" width="34" height="22" rx="3" fill="#CDEED9"/>
      <g stroke="#FFFFFF" stroke-width="3.4" fill="none"><path d="M0 46 L 100 50"/><path d="M30 0 L 36 100"/><path d="M70 50 L 74 100"/></g>'''
        route_l = '<path d="M20 86 C 40 80, 60 66, 100 58" fill="none" stroke="#2563EB" stroke-width="4" stroke-linecap="round"/>'
        route_r = f'<path d="M0 58 C 24 54, 46 44, 62 26" fill="none" stroke="#2563EB" stroke-width="4" stroke-linecap="round"/><circle cx="62" cy="26" r="6.5" fill="{CORAL}" stroke="#FFFFFF" stroke-width="2.6"/>'
    else:
        sw = 7 if size == "32" else 10
        lc = '      <rect width="100" height="100" fill="#FFFFFF"/>\n      <path d="M0 0 H 50 C 46 30, 66 60, 62 100 H 0 Z" fill="#BFDBFE"/>' if size == "32" else '      <rect width="100" height="100" fill="#FFFFFF"/>'
        rc = '      <rect width="100" height="100" fill="#E3EDFD"/>'
        route_l = f'<path d="M14 86 C 40 80, 60 66, 100 58" fill="none" stroke="#2563EB" stroke-width="{sw}" stroke-linecap="round"/>'
        route_r = f'<path d="M0 58 C 24 54, 46 44, 62 26" fill="none" stroke="#2563EB" stroke-width="{sw}" stroke-linecap="round"/><circle cx="62" cy="26" r="{sw*1.5:.0f}" fill="{CORAL}"/>'
    # the whole open map banks like a glider: rotated, and the right wing shaded as it turns away
    body += f'    <g transform="rotate(-13 {C} {C + 40})">\n'
    body += f'    <g transform="{mtx4(Lo, exL, ey)}">\n{lc}\n      {route_l}\n    </g>\n'
    body += f'    <g transform="{mtx4((hx, ht), exR, ey)}">\n{rc}\n      {route_r}\n    </g>\n'
    body += f'    <path d="M{hx} {ht} L {hx} {hb}" stroke="{"#0A1733" if dk else "#1E3A8A"}" stroke-opacity="0.25" stroke-width="{4 if size == "L" else 8}"/>\n'
    body += '    </g>\n'
    return "Wings", defs, body, "url(#bg)"


# -------------------------------------------------------------------- 7 ROUTE N
def route_n_v1(size, dk):
    defs = lg("bg", [(0, "#0F3B3A"), (1, "#0A2726")] if dk else [(0, "#16A085"), (1, "#0E7C6B")], 0, 0, 0.5, 1)
    body = ""
    if size == "L":
        # contour rings, offset so they read as terrain, not as a target
        rings = ""
        for i, r in enumerate([120, 200, 280, 360, 440]):
            rings += f'<ellipse cx="{700 + i*6}" cy="{300 - i*4}" rx="{r*1.15:.0f}" ry="{r:.0f}" transform="rotate(-18 700 300)"/>'
        body += f'    <g fill="none" stroke="#FFFFFF" stroke-opacity="0.14" stroke-width="6">{rings}</g>\n'
        rings = ""
        for r in [90, 160, 230]:
            rings += f'<ellipse cx="250" cy="820" rx="{r*1.3:.0f}" ry="{r:.0f}" transform="rotate(12 250 820)"/>'
        body += f'    <g fill="none" stroke="#FFFFFF" stroke-opacity="0.12" stroke-width="6">{rings}</g>\n'
    # the N: one route with a start point and an arrival point
    x0, x1, y0, y1 = (338, 686, 724, 300) if size == "L" else (322, 702, 744, 286)
    w = {"L": 104, "32": 132, "16": 160}[size]
    d = f"M{x0} {y0} L {x0} {y1} L {x1} {y0} L {x1} {y1}"
    case = "#0B5E52" if not dk else "#06201F"
    if size == "L":
        body += f'    <path d="{d}" fill="none" stroke="{case}" stroke-opacity="0.35" stroke-width="{w+28}" stroke-linejoin="round" stroke-linecap="round"/>\n'
    body += f'    <path d="{d}" fill="none" stroke="#FFFFFF" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="round"/>\n'
    if size == "L":
        body += f'    <path d="{d}" fill="none" stroke="#9FD9CB" stroke-width="8" stroke-dasharray="34 30" stroke-linejoin="round"/>\n'
    # start ring and arrival dot
    r0 = {"L": 30, "32": 40, "16": 0}[size]
    if r0:
        body += f'    <circle cx="{x0}" cy="{y0}" r="{r0}" fill="{"#0E7C6B" if not dk else "#0F3B3A"}"/>\n'
    r1 = {"L": 62, "32": 80, "16": 92}[size]
    body += f'    <circle cx="{x1}" cy="{y1}" r="{r1}" fill="{CORAL}" stroke="#FFFFFF" stroke-width="{r1*0.36:.0f}"/>\n'
    return "Route N", defs, body, "url(#bg)"


# ---------------------------------------------------------------------- 8 PEEK
VIR = [(0.0, (0x44, 0x01, 0x54)), (0.25, (0x3B, 0x52, 0x8B)), (0.5, (0x21, 0x91, 0x8C)), (0.75, (0x5E, 0xC9, 0x62)), (1.0, (0xFD, 0xE7, 0x25))]


def viridis(t):
    t = min(1, max(0, t))
    for (a, ca), (b, cb) in zip(VIR, VIR[1:]):
        if t <= b:
            k = (t - a) / (b - a)
            return "#%02X%02X%02X" % tuple(round(x + (y - x) * k) for x, y in zip(ca, cb))


def peek_v1(size, dk):
    defs = lg("bg", [(0, "#1A2333"), (1, "#121A28")] if dk else [(0, "#EEF1F5"), (1, "#E0E5EC")])
    road = "#2A3548" if dk else "#FFFFFF"
    body = ""
    cx, cy, R = (500, 492, 232) if size == "L" else (512, 500, 268)
    if size == "L":
        body += f'    <g stroke="{road}" stroke-width="22" fill="none" stroke-linecap="round" transform="rotate(-14 512 512)">'
        for x in (150, 330, 690, 870):
            body += f'<path d="M{x} 0 V 1024"/>'
        for y in (180, 350, 670, 850):
            body += f'<path d="M0 {y} H 1024"/>'
        body += '</g>\n'
        body += f'    <path d="M600 924 C 640 820, 760 790, 924 800 V 924 Z" fill="{"#21406A" if dk else "#CBDDF2"}"/>\n'
    defs += f'    <clipPath id="lens"><circle cx="{cx}" cy="{cy}" r="{R}"/></clipPath>\n'
    # inside the lens: the data layer, magnified
    n = 6 if size == "L" else 4
    cs = 2 * R / n
    cells = ""
    for i in range(n):
        for j in range(n):
            u, v = (i + 0.5) / n, (j + 0.5) / n
            val = 0.25 + 0.45 * (u + (1 - v)) / 2 + 0.32 * math.exp(-((u - 0.62) ** 2 + (v - 0.4) ** 2) / 0.06)
            cells += f'<rect x="{f(cx - R + i*cs)}" y="{f(cy - R + j*cs)}" width="{f(cs + 1)}" height="{f(cs + 1)}" fill="{viridis(val)}"/>'
    rim = {"L": 34, "32": 46, "16": 60}[size]
    # lens thickness: an offset disc peeking out at the lower right (no blur, clean edge)
    body += f'    <circle cx="{cx + rim*0.45:.0f}" cy="{cy + rim*0.6:.0f}" r="{R + rim}" fill="{"#0B1220" if dk else "#AEBBCD"}"/>\n'
    body += f'    <g clip-path="url(#lens)">{cells}\n'
    if size != "16":
        sw = 30 if size == "L" else 40
        g = f'      <g stroke="#FFFFFF" stroke-opacity="0.85" stroke-width="{sw}" fill="none" transform="rotate(-14 {cx} {cy})">'
        g += f'<path d="M{cx-R*0.62:.0f} {cy-R-40} V {cy+R+40}"/><path d="M{cx-R-40} {cy+R*0.48:.0f} H {cx+R+40}"/>'
        if size == "L":
            g += f'<path d="M{cx+R*0.78:.0f} {cy-R-40} V {cy+R+40}"/>'
        body += g + '</g>\n'
    body += '    </g>\n'
    rim = {"L": 34, "32": 46, "16": 60}[size]
    body += f'    <circle cx="{cx}" cy="{cy}" r="{R + rim/2}" fill="none" stroke="#FFFFFF" stroke-width="{rim}"/>\n'
    body += f'    <circle cx="{cx}" cy="{cy}" r="{R + rim + 3}" fill="none" stroke="{"#000000" if dk else "#0F172A"}" stroke-opacity="{0.4 if dk else 0.12}" stroke-width="6"/>\n'
    # the clicked feature
    if size != "16":
        pr = 28 if size == "L" else 40
        body += f'    <circle cx="{cx + R*0.3:.0f}" cy="{cy - R*0.12:.0f}" r="{pr}" fill="{CORAL}" stroke="#FFFFFF" stroke-width="{pr*0.45:.0f}"/>\n'
    return "Peek", defs, body, "url(#bg)"


# -------------------------------------------------------------------- 9 RIFFLE
def riffle_v1(size, dk):
    """Three thin map layers fanned from one corner, like a deck flicked open:
    raster at the back, polygons in the middle, streets + a place on top.
    Flat (not isometric) and not fading copies: avoids Nimble Storage and Nimble CRM."""
    defs = lg("bg", [(0, "#0B2A4A"), (1, "#071A30")] if dk else [(0, "#38BDF8"), (1, "#0369A1")], 0, 0, 0.3, 1)
    W = 330 if size == "L" else 360
    px, py = (452, 748) if size == "L" else (446, 772)       # shared pivot = bottom-left corner of every sheet
    rots = (-42, -19, 4) if size == "L" else (-46, -22, 4)
    rx = 30 if size == "L" else 40
    edge = f'stroke="#0F172A" stroke-opacity="{0.35 if dk else 0.14}" stroke-width="{4 if size == "L" else 8}"'
    x0, y0 = px, py - W
    body = ""
    # back: raster layer
    n = 4 if size == "L" else 3
    cs = W / n
    cells = ""
    for i in range(n):
        for j in range(n):
            u, v = (i + 0.5) / n, (j + 0.5) / n
            cells += f'<rect x="{f(x0 + i*cs)}" y="{f(y0 + j*cs)}" width="{f(cs + 1)}" height="{f(cs + 1)}" fill="{viridis(0.3 + 0.55*(u*0.6 + (1-v)*0.4))}"/>'
    defs += f'    <clipPath id="sheet"><rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}"/></clipPath>\n'
    body += f'    <g transform="rotate({rots[0]} {px} {py})"><g clip-path="url(#sheet)">{cells}</g><rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}" fill="none" {edge}/></g>\n'
    # middle: polygon layer
    mid = f'<rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}" fill="#DBEAFE" {edge}/>'
    if size != "16":
        sw = 7 if size == "L" else 12
        mid += f'<g clip-path="url(#sheet)" fill="none" stroke="#7FA6E6" stroke-width="{sw}" stroke-linejoin="round"><path d="M{x0} {y0+W*0.42} L {x0+W*0.4} {y0+W*0.36} L {x0+W*0.62} {y0} M{x0+W*0.4} {y0+W*0.36} L {x0+W*0.5} {y0+W}"/></g>'
    body += f'    <g transform="rotate({rots[1]} {px} {py})">{mid}</g>\n'
    # front: streets + a place
    front = f'<rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}" fill="#FFFFFF" {edge}/>'
    if size == "L":
        front += f'<g clip-path="url(#sheet)"><path d="M{x0} {y0+W*0.70} C {x0+W*0.3} {y0+W*0.62}, {x0+W*0.5} {y0+W*0.82}, {x0+W} {y0+W*0.66} V {y0+W} H {x0} Z" fill="#BFDBFE"/>'
        front += f'<rect x="{x0+W*0.60}" y="{y0+W*0.10}" width="{W*0.28}" height="{W*0.2}" rx="10" fill="#CDEED9"/>'
        front += f'<g stroke="#E2E8F0" stroke-width="14" fill="none"><path d="M{x0} {y0+W*0.40} L {x0+W} {y0+W*0.46}"/><path d="M{x0+W*0.34} {y0} L {x0+W*0.40} {y0+W*0.7}"/></g>'
        front += f'<path d="M{x0+W*0.12} {y0+W*0.86} C {x0+W*0.3} {y0+W*0.7}, {x0+W*0.5} {y0+W*0.5}, {x0+W*0.68} {y0+W*0.3}" stroke="{BLUE}" stroke-width="10" stroke-linecap="round" stroke-dasharray="0.1 26" fill="none"/></g>'
    elif size == "32":
        front += f'<g clip-path="url(#sheet)"><path d="M{x0} {y0+W*0.68} L {x0+W} {y0+W*0.60} V {y0+W} H {x0} Z" fill="#93C5FD"/></g>'
    r = {"L": 34, "32": 50, "16": 66}[size]
    front += f'<circle cx="{f(x0+W*0.68)}" cy="{f(y0+W*0.30)}" r="{r}" fill="{CORAL}" stroke="#FFFFFF" stroke-width="{r*0.4:.0f}"/>'
    body += f'    <g transform="rotate({rots[2]} {px} {py})">{front}</g>\n'
    return "Riffle", defs, body, "url(#bg)"


# ======================================================================= v2
# Round 3 (owner ranking: 1 Peek - chosen, 2 Riffle, 3 Route N).

def peek(size, dk):
    """Peek v2 (chosen): a magnifier, ring + handle in one solid colour, over a calm
    street map; inside, a stepped raster patch and the coral clicked feature.
    No crosshair (the handle and the off-centre feature keep it from reading as a scope).
    Sizes: "L" 1024 master, "32", "16" hand-drawn small art, "site" = website header (~30 px)."""
    small = size in ("32", "16", "site")
    defs = lg("bg", [(0, "#1B2536"), (1, "#111827")] if dk else [(0, "#F1F4F8"), (1, "#E1E7EF")])
    ring = "#E2E8F0" if dk else "#1E3A8A"
    road = "#273349" if dk else "#FFFFFF"
    water = "#1F3F66" if dk else "#C9DCF2"
    cx, cy, R, rim, Lh, hw = {
        "L": (446, 446, 206, 46, 250, 92),
        "32": (436, 436, 214, 72, 240, 116),
        "site": (432, 432, 214, 80, 236, 124),
        "16": (430, 430, 206, 96, 226, 140),
    }[size]
    body = ""
    # outside: the base map (thick, few streets so nothing shimmers at 64-128 px)
    if size == "L":
        body += f'    <path d="M100 640 C 190 650, 250 720, 262 924 H 100 Z" fill="{water}"/>\n'
        body += f'    <g stroke="{road}" stroke-width="34" fill="none" transform="rotate(-14 512 512)"><path d="M170 0 V 1024"/><path d="M860 0 V 1024"/><path d="M0 210 H 1024"/><path d="M0 860 H 1024"/></g>\n'
    elif size == "32":
        body += f'    <path d="M100 620 C 200 640, 260 720, 270 924 H 100 Z" fill="{water}"/>\n'
    # inside the lens: the data layer
    defs += f'    <clipPath id="lens"><circle cx="{cx}" cy="{cy}" r="{R + 2}"/></clipPath>\n'
    inner = ""
    if size == "16":
        inner += f'<rect x="{cx-R-4}" y="{cy-R-4}" width="{2*R+8}" height="{2*R+8}" fill="#21918C"/>'
        inner += f'<path d="M{cx-R} {cy+R*0.1} L {cx+R*0.05} {cy-R*0.05} L {cx+R*0.25} {cy+R} L {cx-R} {cy+R} Z" fill="#7AD151"/>'
    else:
        n = 4 if size == "L" else 3
        cs = 2 * R / n
        steps = {4: [[0.30, 0.36, 0.50, 0.62], [0.34, 0.46, 0.66, 0.80], [0.42, 0.60, 0.88, 0.72], [0.50, 0.70, 0.76, 0.58]],
                 3: [[0.32, 0.48, 0.64], [0.42, 0.70, 0.90], [0.56, 0.84, 0.68]]}[n]
        for i in range(n):
            for j in range(n):
                inner += f'<rect x="{f(cx - R + i*cs)}" y="{f(cy - R + j*cs)}" width="{f(cs + 1)}" height="{f(cs + 1)}" fill="{viridis(steps[j][i])}"/>'
        if size == "L":
            # one magnified street, not through the centre
            inner += f'<path d="M{cx-R} {cy+R*0.42} C {cx-R*0.3} {cy+R*0.30}, {cx+R*0.2} {cy+R*0.62}, {cx+R} {cy+R*0.48}" stroke="#FFFFFF" stroke-opacity="0.9" stroke-width="30" fill="none"/>'
    body += f'    <g clip-path="url(#lens)">{inner}</g>\n'
    # the clicked feature (coral, white ring) - off-centre, upper right
    fr = {"L": 36, "32": 52, "site": 56, "16": 64}[size]
    fx, fy = (cx + R * 0.30, cy - R * 0.22) if size != "16" else (cx + R * 0.22, cy - R * 0.18)
    body += f'    <circle cx="{f(fx)}" cy="{f(fy)}" r="{fr}" fill="{CORAL}" stroke="#FFFFFF" stroke-width="{fr*0.42:.0f}"/>\n'
    # handle (drawn first under the ring end) and ring
    c = math.cos(math.radians(45))
    r0 = R + rim * 0.5
    body += f'    <path d="M{f(cx + r0*c)} {f(cy + r0*c)} L {f(cx + (r0+Lh)*c)} {f(cy + (r0+Lh)*c)}" stroke="{ring}" stroke-width="{hw}" stroke-linecap="round"/>\n'
    body += f'    <circle cx="{cx}" cy="{cy}" r="{R + rim/2}" fill="none" stroke="{ring}" stroke-width="{rim}"/>\n'
    if size == "L":
        # a quiet glint on the glass: one flat arc, not a gradient or blur
        body += f'    <path d="M{f(cx - R*0.72)} {f(cy - R*0.30)} A {R*0.78:.0f} {R*0.78:.0f} 0 0 1 {f(cx - R*0.28)} {f(cy - R*0.72)}" stroke="#FFFFFF" stroke-opacity="0.75" stroke-width="22" stroke-linecap="round" fill="none"/>\n'
    return "Peek", defs, body, "url(#bg)"


def riffle(size, dk):
    """Riffle v2: three big thin map sheets fanned from the bottom-left corner, filling the squircle."""
    defs = lg("bg", [(0, "#0B2A4A"), (1, "#071A30")] if dk else [(0, "#38BDF8"), (1, "#0369A1")], 0, 0, 0.3, 1)
    W, px, py, rots = {"L": (520, 340, 782, (-22, -11, 0)),
                       "32": (540, 338, 796, (-24, -12, 0)),
                       "16": (540, 342, 800, (-25, -12.5, 0))}[size]
    rx = {"L": 34, "32": 44, "16": 52}[size]
    ew = {"L": 6, "32": 12, "16": 16}[size]
    edge = f'stroke="#0F172A" stroke-opacity="{0.4 if dk else 0.2}" stroke-width="{ew}"'
    x0, y0 = px, py - W
    defs += f'    <clipPath id="sheet"><rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}"/></clipPath>\n'
    body = ""
    # back: raster
    n = {"L": 6, "32": 3, "16": 2}[size]
    cs = W / n
    cells = ""
    for i in range(n):
        for j in range(n):
            u, v = (i + 0.5) / n, (j + 0.5) / n
            t = 0.28 + 0.62 * (0.55 * (1 - u) + 0.45 * (1 - v)) + 0.06 * math.sin(7 * u + 3 * v)
            if n == 2:
                t = [[0.85, 0.62], [0.55, 0.40]][j][i]
            cells += f'<rect x="{f(x0 + i*cs)}" y="{f(y0 + j*cs)}" width="{f(cs + 1)}" height="{f(cs + 1)}" fill="{viridis(t)}"/>'
    body += f'    <g transform="rotate({rots[0]} {px} {py})"><g clip-path="url(#sheet)">{cells}</g><rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}" fill="none" {edge}/></g>\n'
    # middle: polygons (only the left/top wedge shows, so the content sits there)
    mid = f'<rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}" fill="#DBEAFE" {edge}/>'
    if size != "16":
        mid += f'<g clip-path="url(#sheet)"><path d="M{x0} {y0} H {x0+W*0.34} L {x0+W*0.22} {y0+W*0.36} L {x0} {y0+W*0.44} Z" fill="#B7CCF4"/>'
        sw = 9 if size == "L" else 16
        mid += f'<path d="M{x0} {y0+W*0.44} L {x0+W*0.22} {y0+W*0.36} L {x0+W*0.34} {y0} M{x0+W*0.22} {y0+W*0.36} L {x0+W*0.30} {y0+W} M{x0} {y0+W*0.76} L {x0+W*0.26} {y0+W*0.70}" stroke="#6C93D9" stroke-width="{sw}" fill="none" stroke-linejoin="round"/></g>'
    body += f'    <g transform="rotate({rots[1]} {px} {py})">{mid}</g>\n'
    # front: streets, sea, park, route and the coral place
    fr = f'<rect x="{x0}" y="{y0}" width="{W}" height="{W}" rx="{rx}" fill="#FFFFFF" {edge}/>'
    if size == "L":
        fr += f'<g clip-path="url(#sheet)"><path d="M{x0} {y0+W*0.72} C {x0+W*0.3} {y0+W*0.64}, {x0+W*0.52} {y0+W*0.84}, {x0+W} {y0+W*0.68} V {y0+W} H {x0} Z" fill="#BFDBFE"/>'
        fr += f'<rect x="{x0+W*0.60}" y="{y0+W*0.09}" width="{W*0.28}" height="{W*0.19}" rx="14" fill="#CDEED9"/>'
        fr += f'<g stroke="#E3E8EF" stroke-width="28" fill="none"><path d="M{x0} {y0+W*0.40} L {x0+W} {y0+W*0.46}"/><path d="M{x0+W*0.34} {y0} L {x0+W*0.40} {y0+W*0.7}"/></g>'
        fr += f'<path d="M{x0+W*0.12} {y0+W*0.86} C {x0+W*0.3} {y0+W*0.7}, {x0+W*0.5} {y0+W*0.52}, {x0+W*0.62} {y0+W*0.36}" stroke="{BLUE}" stroke-width="16" stroke-linecap="round" stroke-dasharray="0.1 34" fill="none"/></g>'
    elif size == "32":
        fr += f'<g clip-path="url(#sheet)"><path d="M{x0} {y0+W*0.70} L {x0+W} {y0+W*0.62} V {y0+W} H {x0} Z" fill="#93C5FD"/><path d="M{x0} {y0+W*0.40} L {x0+W} {y0+W*0.46}" stroke="#E3E8EF" stroke-width="36"/></g>'
    else:
        fr += f'<g clip-path="url(#sheet)"><path d="M{x0} {y0+W*0.70} L {x0+W} {y0+W*0.62} V {y0+W} H {x0} Z" fill="#93C5FD"/></g>'
    r = {"L": 44, "32": 64, "16": 80}[size]
    fr += f'<circle cx="{f(x0+W*0.68)}" cy="{f(y0+W*0.30)}" r="{r}" fill="{CORAL}" stroke="#FFFFFF" stroke-width="{r*0.4:.0f}"/>'
    body += f'    <g transform="rotate({rots[2]} {px} {py})">{fr}</g>\n'
    return "Riffle", defs, body, "url(#bg)"


def route_n(size, dk):
    """Route N v2: a real map (land, lake, hill with layered contours, streets) with the N as a road in it."""
    defs = lg("bg", [(0, "#1A2520"), (1, "#141D19")] if dk else [(0, "#F1F4E8"), (1, "#E6ECDA")])
    water = "#1F4A73" if dk else "#8EC3EE"
    street = "#2B3A33" if dk else "#FFFFFF"
    hills = ["#21352A", "#284434", "#30553F"] if dk else ["#D6EAC6", "#BFDFA9", "#A5D18A"]
    cline = "#4F7F5C" if dk else "#6FA95C"
    road, casing = ("#2DD4BF", "#0B1411") if dk else ("#0F766E", "#FFFFFF")
    x0, x1, y0, y1 = (338, 686, 724, 300) if size == "L" else (322, 702, 744, 286)
    body = ""
    # lake (bottom right) and hill (top left) are big enough to read at 16 px
    body += f'    <path d="M924 548 C 840 572, 790 650, 812 736 S 760 880, 640 924 H 924 Z" fill="{water}"/>\n'
    if size == "L":
        body += f'    <path d="M924 548 C 840 572, 790 650, 812 736 S 760 880, 640 924" fill="none" stroke="{"#5E9BD0" if not dk else "#2F6496"}" stroke-width="10"/>\n'
    hc = (290, 300)
    radii = [(210, 168), (140, 112), (74, 58)] if size == "L" else ([(220, 176), (124, 98)] if size == "32" else [(220, 176)])
    for k, (a, b) in enumerate(radii):
        cs = f' stroke="{cline}" stroke-width="10"' if size == "L" else ""
        body += f'    <ellipse cx="{hc[0] + k*14}" cy="{hc[1] + k*8}" rx="{a}" ry="{b}" transform="rotate(-20 {hc[0]} {hc[1]})" fill="{hills[k]}"{cs}/>\n'
    # streets: a visible grid that the N road crosses
    if size == "L":
        body += f'    <g stroke="{street}" stroke-width="24" fill="none"><path d="M100 484 H 924"/><path d="M100 850 H 700"/><path d="M512 100 V 924"/><path d="M842 100 V 540"/><path d="M190 560 V 924"/></g>\n'
    elif size == "32":
        body += f'    <g stroke="{street}" stroke-width="40" fill="none"><path d="M100 500 H 924"/><path d="M512 100 V 924"/></g>\n'
    # the N as a road in the map: casing + fill
    d = f"M{x0} {y0} L {x0} {y1} L {x1} {y0} L {x1} {y1}"
    if size == "16":
        # at 16 px a thin route would vanish: the road itself carries the teal
        body += f'    <path d="{d}" fill="none" stroke="{casing}" stroke-width="192" stroke-linejoin="round" stroke-linecap="round"/>\n'
        body += f'    <path d="{d}" fill="none" stroke="{road}" stroke-width="140" stroke-linejoin="round" stroke-linecap="round"/>\n'
    else:
        # a major road (map style: pale fill, darker casing) with the route drawn along it
        w, rw = (92, 40) if size == "L" else (124, 60)
        rc = "#9DAE95" if not dk else "#0B1411"
        rf = "#FFFFFF" if not dk else "#3A4A42"
        body += f'    <path d="{d}" fill="none" stroke="{rc}" stroke-width="{w + 20}" stroke-linejoin="round" stroke-linecap="round"/>\n'
        body += f'    <path d="{d}" fill="none" stroke="{rf}" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="round"/>\n'
        body += f'    <path d="{d}" fill="none" stroke="{road}" stroke-width="{rw}" stroke-linejoin="round" stroke-linecap="round"/>\n'
    # start pin and coral arrival dot
    if size != "16":
        R, L = (56, 136) if size == "L" else (76, 176)
        pc = "#1E3A8A" if not dk else "#93C5FD"
        body += f'    <path d="{pin_d(x0, y0 - L, R, L)}" fill="{pc}" stroke="{casing}" stroke-width="{12 if size == "L" else 18}" stroke-linejoin="round"/>\n'
        body += f'    <circle cx="{x0}" cy="{y0 - L}" r="{R*0.4:.0f}" fill="{casing if not dk else "#0B1411"}"/>\n'
    r1 = {"L": 64, "32": 84, "16": 100}[size]
    body += f'    <circle cx="{x1}" cy="{y1}" r="{r1}" fill="{CORAL}" stroke="{casing}" stroke-width="{r1*0.36:.0f}"/>\n'
    return "Route N", defs, body, "url(#bg)"


V1 = [("7", "route-n", route_n_v1), ("8", "peek", peek_v1), ("9", "riffle", riffle_v1)]


CONCEPTS = [("1", "dart", dart), ("2", "feather", feather), ("3", "lift", lift), ("4", "hover", hover),
            ("5", "hop", hop), ("6", "wings", wings), ("7", "route-n", route_n), ("8", "peek", peek), ("9", "riffle", riffle)]


def wrap(title, defs, body, bg, dk):
    rim = ('stroke="#FFFFFF" stroke-opacity="0.12"' if dk else 'stroke="#0F172A" stroke-opacity="0.08"')
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <title>NimbleMap - {title}</title>
  <defs>
    <path id="sq" d="{SQ}"/>
    <clipPath id="body"><use href="#sq"/></clipPath>
{defs}  </defs>
  <!-- container: macOS squircle, 824 px body centred on 1024 canvas -->
  <use href="#sq" fill="{bg}"/>
  <g clip-path="url(#body)">
{body}  </g>
  <use href="#sq" fill="none" {rim} stroke-width="2"/>
</svg>
"""


def prefix(svg, p):
    for i in sorted(set(re.findall(r'id="([^"]+)"', svg)), key=len, reverse=True):
        svg = svg.replace(f'id="{i}"', f'id="{p}-{i}"').replace(f"url(#{i})", f"url(#{p}-{i})").replace(f'href="#{i}"', f'href="#{p}-{i}"')
    return svg


def to_mono(svg):
    """Tinted appearance: luminance of every colour (system applies the tint)."""
    def g(m):
        h = m.group(1)
        r, gg, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
        y = round(0.2126 * r + 0.7152 * gg + 0.0722 * b)
        return "#%02X%02X%02X" % (y, y, y)
    return re.sub(r"#([0-9A-Fa-f]{6})\b", g, svg)


if __name__ == "__main__":
    os.makedirs(f"{D}/variants", exist_ok=True)
    for n, name, fn in CONCEPTS:
        base = f"concept-{n}-{name}"
        for size, suffix in (("L", ""), ("32", "-32"), ("16", "-16")):
            t, de, bo, bg = fn(size, False)
            if size != "L":
                t += f" ({size} px)"
            open(f"{D}/{base}{suffix}.svg", "w").write(prefix(wrap(t, de, bo, bg, False), f"n{n}{suffix.replace('-', 'x')}"))
        t, de, bo, bg = fn("L", True)
        dark = wrap(t + " (dark)", de, bo, bg, True)
        open(f"{D}/variants/{base}-dark.svg", "w").write(prefix(dark, f"n{n}d"))
        open(f"{D}/variants/{base}-tinted.svg", "w").write(prefix(to_mono(dark).replace("(dark)", "(tinted)"), f"n{n}t"))
    # v1 of the three revised concepts, kept for comparison
    os.makedirs(f"{D}/v1", exist_ok=True)
    for n, name, fn in V1:
        base = f"concept-{n}-{name}-v1"
        for size, suffix in (("L", ""), ("32", "-32"), ("16", "-16")):
            t, de, bo, bg = fn(size, False)
            open(f"{D}/v1/{base}{suffix}.svg", "w").write(prefix(wrap(t + " v1", de, bo, bg, False), f"v{n}{suffix.replace('-', 'x')}"))
        t, de, bo, bg = fn("L", True)
        open(f"{D}/v1/{base}-dark.svg", "w").write(prefix(wrap(t + " v1 (dark)", de, bo, bg, True), f"v{n}d"))
    # website header mark (replaces web/lm/assets/logo.svg; tuned for ~30 px)
    t, de, bo, bg = peek("site", False)
    open(f"{D}/peek-site.svg", "w").write(prefix(wrap("Peek (website, 30 px)", de, bo, bg, False), "nmsite"))
    print("ok", len(CONCEPTS))
