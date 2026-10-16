"""Generate LayeredMap logo concept SVGs with computed geometry."""
import math, os

OUT = "/Volumes/LaCie/repos/LayeredMap/design/logo"
C = 512          # canvas centre
B0, B1 = 100, 924  # macOS icon body (824 px)
A = 412          # body half-size


def f(v):
    s = f"{v:.1f}"
    return s[:-2] if s.endswith(".0") else s


def pts(seq):
    return " ".join(f"{f(x)},{f(y)}" for x, y in seq)


def squircle_d(n=5.0, a=A, cx=C, cy=C, steps=36):
    """Superellipse |x|^n+|y|^n=a^n approximating the macOS icon shape."""
    out = []
    for i in range(steps * 4):
        t = 2 * math.pi * i / (steps * 4)
        ct, st = math.cos(t), math.sin(t)
        x = cx + a * math.copysign(abs(ct) ** (2 / n), ct)
        y = cy + a * math.copysign(abs(st) ** (2 / n), st)
        out.append((x, y))
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in out) + " Z"


SQ = squircle_d()


def wrap(name, defs, body, bg):
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <title>LayeredMap - {name}</title>
  <defs>
    <path id="sq" d="{SQ}"/>
    <clipPath id="body"><use href="#sq"/></clipPath>
{defs}
  </defs>
  <!-- container: macOS squircle, 824 px body centred on 1024 canvas -->
  <use href="#sq" fill="{bg}"/>
  <g clip-path="url(#body)">
{body}
  </g>
  <use href="#sq" fill="none" stroke="#0F172A" stroke-opacity="0.08" stroke-width="2"/>
</svg>
"""


def lingrad(id_, stops, x1=0, y1=0, x2=0, y2=1, units=None):
    u = f' gradientUnits="{units}"' if units else ""
    s = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in stops)
    return f'    <linearGradient id="{id_}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"{u}>{s}</linearGradient>\n'


# ------------------------------------------------------------------ 1 STRATA
def strata():
    w = 272
    h = w * math.tan(math.radians(30))  # true isometric
    t = 16                                # sheet thickness
    gap = 112
    cys = [C - gap - 6, C - 6, C + gap - 6]  # top, mid, bottom sheets

    def m(cy):
        return f"matrix({f(w/100)},{f(h/100)},{f(-w/100)},{f(h/100)},{C},{f(cy-h)})"

    def edge(cy, col):
        L, Bm, R = (C - w, cy), (C, cy + h), (C + w, cy)
        return f'    <polygon points="{pts([L, Bm, R, (R[0], R[1]+t), (Bm[0], Bm[1]+t), (L[0], L[1]+t)])}" fill="{col}"/>\n'

    defs = lingrad("bg", [(0, "#F8FAFC"), (1, "#E3E9F1")])
    defs += lingrad("rast", [(0, "#3B528B"), (0.45, "#21918C"), (1, "#7AD151")], 0, 0, 1, 1)
    defs += '    <radialGradient id="hot" cx="0.86" cy="0.86" r="0.28"><stop offset="0" stop-color="#FDE725" stop-opacity="0.75"/><stop offset="1" stop-color="#FDE725" stop-opacity="0"/></radialGradient>\n'
    defs += lingrad("topg", [(0, "#3B82F6"), (1, "#2563EB")], 0, 0, 1, 1)
    body = ""
    # bottom: raster
    cy = cys[2]
    body += "    <!-- bottom sheet: raster (viridis) -->\n" + edge(cy, "#1F6F78")
    body += f'    <g transform="{m(cy)}"><rect width="100" height="100" fill="url(#rast)"/><rect width="100" height="100" fill="url(#hot)"/></g>\n'
    # middle: polygons
    cy = cys[1]
    body += "    <!-- middle sheet: polygons (districts) -->\n" + edge(cy, "#B8C4D3")
    body += f'''    <g transform="{m(cy)}">
      <rect width="100" height="100" fill="#FFFFFF"/>
      <polygon points="56,58 80,52 88,100 64,100" fill="#C7D8FD"/>
      <polygon points="0,62 28,66 22,100 0,100" fill="#E4ECFA"/>
      <g fill="none" stroke="#9AABC2" stroke-width="1.5" stroke-linejoin="round">
        <polyline points="0,44 30,40 56,58 80,52 100,56"/>
        <polyline points="30,40 36,0"/><polyline points="0,62 28,66 56,58"/>
        <polyline points="28,66 22,100"/><polyline points="56,58 64,100"/><polyline points="80,52 88,100"/>
      </g>
    </g>
'''
    # top: points
    cy = cys[0]
    body += "    <!-- top sheet: points -->\n" + edge(cy, "#1D4ED8")
    body += f'''    <g transform="{m(cy)}">
      <rect width="100" height="100" fill="url(#topg)"/>
      <g fill="#FFFFFF"><circle cx="30" cy="34" r="6"/><circle cx="66" cy="28" r="6"/><circle cx="58" cy="70" r="6"/></g>
      <circle cx="58" cy="70" r="11" fill="none" stroke="#FFFFFF" stroke-opacity="0.45" stroke-width="2.4"/>
    </g>
'''
    return wrap("Strata", defs, body, "url(#bg)")


# ------------------------------------------------------------------ 2 OVERLAY
def overlay():
    defs = lingrad("bg", [(0, "#F8FAFC"), (1, "#E6ECF3")])
    ca = (428, 440); ra = 206
    poly = [(560, 404), (792, 470), (780, 742), (498, 790), (452, 598)]
    pd = "M" + " L".join(f"{x} {y}" for x, y in poly) + " Z"
    defs += f'    <clipPath id="ca"><circle cx="{ca[0]}" cy="{ca[1]}" r="{ra}"/></clipPath>\n'
    defs += f'    <path id="dist" d="{pd}" stroke-width="64" stroke-linejoin="round"/>\n'
    body = f'''    <!-- faint graticule -->
    <g stroke="#CBD5E1" stroke-width="3" stroke-opacity="0.55">
      <line x1="100" y1="306" x2="924" y2="306"/><line x1="100" y1="512" x2="924" y2="512"/><line x1="100" y1="718" x2="924" y2="718"/>
      <line x1="306" y1="100" x2="306" y2="924"/><line x1="512" y1="100" x2="512" y2="924"/><line x1="718" y1="100" x2="718" y2="924"/>
    </g>
    <!-- layer A: buffer (circle) -->
    <circle cx="{ca[0]}" cy="{ca[1]}" r="{ra}" fill="#9CC3FA"/>
    <!-- layer B: district (polygon) -->
    <use href="#dist" fill="#9FDAB9" stroke="#9FDAB9"/>
    <!-- intersection = the spatial relationship -->
    <g clip-path="url(#ca)"><use href="#dist" fill="#2563EB" stroke="#2563EB"/></g>
    <!-- feature in the intersection -->
    <circle cx="546" cy="548" r="30" fill="#FFFFFF"/>
'''
    return wrap("Overlay", defs, body, "url(#bg)")


# ------------------------------------------------------------------ 3 FOLD
def fold():
    pw, H = 226, 452
    x0 = C - 1.5 * pw
    ytop = [C - H/2 + 24, C - H/2 - 24, C - H/2 + 24, C - H/2 - 24]
    defs = lingrad("bg", [(0, "#F8FAFC"), (1, "#E6ECF3")])
    defs += f'    <clipPath id="pc"><rect width="{pw}" height="{H}"/></clipPath>\n'
    defs += lingrad("shadeL", [(0, "#000000"), (1, "#000000")])
    route = f"M -10 372 C 150 330, 250 140, 372 214 S 520 330, {3*pw - 70} 150"
    fills = ["#DCEFDD", "#C6D7F2", "#F6EBD2"]
    layer = [
        # terrain contours
        '<g fill="none" stroke="#8CC496" stroke-width="12"><ellipse cx="104" cy="250" rx="96" ry="120"/><ellipse cx="110" cy="262" rx="60" ry="76"/><ellipse cx="116" cy="272" rx="24" ry="32"/></g>',
        # districts
        '<polygon points="0,120 120,150 132,300 0,330" fill="#AFC6EE"/><g fill="none" stroke="#FFFFFF" stroke-width="9" stroke-linejoin="round"><polyline points="0,120 120,150 226,110"/><polyline points="120,150 132,300 226,320"/><polyline points="0,330 132,300 110,452"/></g>',
        # points
        '<g fill="#E69F00"><circle cx="186" cy="262" r="20"/><circle cx="150" cy="370" r="20"/><circle cx="70" cy="90" r="20"/></g>',
    ]
    body = ""
    for i in range(3):
        xa = x0 + i * pw
        slope = (ytop[i+1] - ytop[i]) / pw
        body += f'    <!-- panel {i+1} -->\n    <g transform="matrix(1,{f(slope*1000)/1000 if False else round(slope,5)},0,1,{f(xa)},{f(ytop[i])})">\n'
        body += f'      <g clip-path="url(#pc)">\n        <rect width="{pw}" height="{H}" fill="{fills[i]}"/>\n        {layer[i]}\n'
        body += f'        <path d="{route}" transform="translate({-i*pw},0)" fill="none" stroke="#FFFFFF" stroke-width="44" stroke-linecap="round"/>\n'
        body += f'        <path d="{route}" transform="translate({-i*pw},0)" fill="none" stroke="#2563EB" stroke-width="28" stroke-linecap="round"/>\n'
        if i == 2:
            ex, ey = 3*pw - 70 - 2*pw, 150
            body += f'        <circle cx="{ex}" cy="{ey}" r="30" fill="#2563EB"/><circle cx="{ex}" cy="{ey}" r="12" fill="#FFFFFF"/>\n'
        shade = 0.10 if i == 1 else 0.0
        if shade:
            body += f'        <rect width="{pw}" height="{H}" fill="#0F172A" fill-opacity="{shade}"/>\n'
        body += '      </g>\n    </g>\n'
    # fold creases highlight
    for i in (1, 2):
        xa = x0 + i * pw
        body += f'    <line x1="{f(xa)}" y1="{f(ytop[i])}" x2="{f(xa)}" y2="{f(ytop[i]+H)}" stroke="#FFFFFF" stroke-opacity="0.6" stroke-width="3"/>\n'
    return wrap("Fold", defs, body, "url(#bg)")


# ------------------------------------------------------------------ 4 STRATA PIN
def pin():
    cx, cy, R = C, 418, 204
    tip = 818
    d = tip - cy
    alpha = math.asin(R / d)              # half-angle at tip
    beta = math.pi/2 - alpha              # angle of tangent point from axis
    tx = R * math.sin(math.pi - beta)     # tangent offsets
    # tangent point: angle measured from downward axis = beta... compute directly
    ang = math.acos(R / d)
    tpx, tpy = R * math.sin(ang), cy + R * math.cos(ang)
    # rounded tip radius
    rt = 30
    dc = rt / math.sin(alpha)             # tip-circle centre distance from tip
    tcy = tip - dc
    # tangent points on the rounded tip circle
    ex = rt * math.cos(alpha); ey = tcy + rt * math.sin(alpha)
    pd = (f"M {f(cx+tpx)} {f(tpy)} L {f(cx+ex)} {f(ey)} "
          f"A {rt} {rt} 0 0 1 {f(cx-ex)} {f(ey)} L {f(cx-tpx)} {f(tpy)} "
          f"A {R} {R} 0 1 1 {f(cx+tpx)} {f(tpy)} Z")
    top_y = cy - R
    defs = lingrad("bg", [(0, "#F8FAFC"), (1, "#E6ECF3")])
    defs += f'    <clipPath id="pinc"><path d="{pd}"/></clipPath>\n'
    cuts = [(472, 30), (640, 24)]
    g = 11  # half gap between strata
    def band(ya, sa, yb, sb):
        top = f"M 250 {ya+g} Q {C} {ya+g+2*sa} 774 {ya+g}" if ya else "M 250 150 L 774 150"
        bot = f"L 774 {yb-g} Q {C} {yb-g+2*sb} 250 {yb-g} Z" if yb else "L 774 900 L 250 900 Z"
        return top + " " + bot
    b1 = band(None, 0, cuts[0][0], cuts[0][1])
    b2 = band(cuts[0][0], cuts[0][1], cuts[1][0], cuts[1][1])
    b3 = band(cuts[1][0], cuts[1][1], None, 0)
    body = f'''    <!-- quiet street grid -->
    <g stroke="#FFFFFF" stroke-width="16" fill="none">
      <line x1="100" y1="250" x2="924" y2="330"/><line x1="100" y1="720" x2="924" y2="640"/>
      <line x1="230" y1="100" x2="300" y2="924"/><line x1="800" y1="100" x2="730" y2="924"/>
    </g>
    <ellipse cx="{C}" cy="{tip+6}" rx="118" ry="24" fill="#0F172A" fill-opacity="0.10"/>
    <!-- pin made of three strata (points / polygons / raster) -->
    <g clip-path="url(#pinc)">
      <path d="{b1}" fill="#2563EB"/>
      <path d="{b2}" fill="#3E9BD6"/>
      <path d="{b3}" fill="#4DB88C"/>
    </g>
    <circle cx="{cx}" cy="{cy-28}" r="62" fill="#FFFFFF"/>
'''
    return wrap("Strata Pin", defs, body, "url(#bg)")


# ------------------------------------------------------------------ 5 SWIPE
def swipe():
    D = {
        "D1": [(100, 100), (300, 100), (330, 400), (100, 430)],
        "D2": [(300, 100), (690, 100), (700, 441), (560, 460), (330, 400)],
        "D3": [(690, 100), (924, 100), (924, 410), (760, 432), (700, 441)],
        "D4": [(100, 430), (330, 400), (440, 429), (470, 924), (100, 924)],
        "D5": [(440, 429), (560, 460), (700, 441), (760, 432), (800, 700), (780, 924), (470, 924)],
        "D6": [(760, 432), (924, 410), (924, 720), (800, 700)],
        "D7": [(800, 700), (924, 720), (924, 924), (780, 924)],
    }
    col = {"D1": "#3E5C9A", "D2": "#33708F", "D3": "#2A8E92", "D4": "#5CBF7A",
           "D5": "#3FA884", "D6": "#8CCF63", "D7": "#C3DE57"}
    defs = '    <clipPath id="L"><rect x="0" y="0" width="512" height="1024"/></clipPath>\n'
    defs += '    <clipPath id="R"><rect x="512" y="0" width="512" height="1024"/></clipPath>\n'
    river = "M 80 640 C 300 520, 520 760, 944 600"
    lines = "".join(f'<polygon points="{pts(p)}"/>' for p in D.values())
    left = f'''    <!-- left: base map -->
    <g clip-path="url(#L)">
      <rect width="1024" height="1024" fill="#F3F5F8"/>
      <polygon points="{pts(D['D4'])}" fill="#DCEFE0"/>
      <g fill="none" stroke="#CDD6E1" stroke-width="7" stroke-linejoin="round">{lines}</g>
      <path d="{river}" fill="none" stroke="#A9CBF5" stroke-width="48" stroke-linecap="round"/>
    </g>
'''
    fills = "".join(f'<polygon points="{pts(p)}" fill="{col[k]}"/>' for k, p in D.items())
    right = f'''    <!-- right: same geography as a data layer -->
    <g clip-path="url(#R)">
      {fills}
      <g fill="none" stroke="#FFFFFF" stroke-opacity="0.55" stroke-width="7" stroke-linejoin="round">{lines}</g>
      <path d="{river}" fill="none" stroke="#DCEBFF" stroke-width="48" stroke-linecap="round"/>
    </g>
'''
    handle = f'''    <!-- swipe divider + handle -->
    <rect x="504" y="0" width="16" height="1024" fill="#FFFFFF"/>
    <circle cx="512" cy="518" r="78" fill="#0F172A" fill-opacity="0.12"/>
    <circle cx="512" cy="512" r="76" fill="#FFFFFF"/>
    <g fill="none" stroke="#2563EB" stroke-width="16" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="494,482 466,512 494,542"/><polyline points="530,482 558,512 530,542"/>
    </g>
'''
    return wrap("Swipe", defs, left + right + handle, "#F3F5F8")


# ------------------------------------------------------------------ 6 PEEL
def peel():
    k = 396
    defs = lingrad("sheet", [(0, "#3B82F6"), (1, "#1D4ED8")], 0, 0, 1, 1)
    defs += lingrad("rast", [(0, "#21918C"), (0.55, "#5EC962"), (1, "#D7E34A")], f(k), 0, 1024, f(-k+0), "userSpaceOnUse")
    defs += lingrad("under", [(0, "#FFFFFF"), (1, "#DCE4EE")], 0, 0, 1, 1)
    half = f'<polygon points="{k},0 1100,0 1100,{1100-k}"/>'
    defs += f'    <clipPath id="corner">{half}</clipPath>\n'
    defs += f'    <clipPath id="rest"><polygon points="{k},0 -100,0 -100,1124 1124,1124 1124,{1024-k}"/></clipPath>\n'
    refl = f"matrix(0,1,1,0,{k},{-k})"
    body = f'''    <!-- top layer: base map -->
    <g clip-path="url(#rest)">
      <rect width="1024" height="1024" fill="url(#sheet)"/>
      <g fill="none" stroke="#FFFFFF" stroke-opacity="0.22" stroke-width="10" stroke-linejoin="round">
        <polyline points="100,420 330,390 470,470 640,430"/><polyline points="330,390 300,100"/>
        <polyline points="470,470 500,924"/><polyline points="100,700 300,680 500,720 924,660"/>
        <polyline points="760,450 780,924"/>
      </g>
      <g fill="#FFFFFF"><circle cx="270" cy="560" r="24"/><circle cx="640" cy="800" r="24"/><circle cx="380" cy="820" r="24"/></g>
    </g>
    <!-- revealed layer beneath: raster -->
    <g clip-path="url(#corner)">
      <rect width="1024" height="1024" fill="url(#rast)"/>
      <g fill="none" stroke="#FFFFFF" stroke-opacity="0.35" stroke-width="6">
        <path d="M 640 100 Q 760 260 924 280"/><path d="M 760 100 Q 840 190 924 200"/>
      </g>
    </g>
    <!-- shadow cast by the curled flap -->
    <g transform="translate(-10,14)" opacity="0.22"><g transform="{refl}"><use href="#sq" clip-path="url(#corner)" fill="#0B1E5B"/></g></g>
    <!-- the peeled flap (underside of top sheet) -->
    <g transform="{refl}"><use href="#sq" clip-path="url(#corner)" fill="url(#under)"/></g>
'''
    return wrap("Peel", defs, body, "#1D4ED8")


CONCEPTS = [
    ("1", "strata", strata),
    ("2", "overlay", overlay),
    ("3", "fold", fold),
    ("4", "strata-pin", pin),
    ("5", "swipe", swipe),
    ("6", "peel", peel),
]

if __name__ == "__main__":
    for n, name, fn in CONCEPTS:
        p = os.path.join(OUT, f"concept-{n}-{name}.svg")
        with open(p, "w") as fh:
            fh.write(fn())
        print(p)
