"""strata-raster: the original Strata (concept-1) with a pixelated raster bottom sheet.

Reads concept-1-strata.svg, prefixes ids, swaps only the bottom sheet's fill for an
N x N grid of viridis cells sampled from a smooth field with a warm spot.
Usage: python gen_raster.py [N_main N_32 N_16]
"""
import math, re, sys

D = "/Volumes/LaCie/repos/LayeredMap/design/logo"
VIR = [(0.0, (0x44, 0x01, 0x54)), (0.25, (0x3B, 0x52, 0x8B)), (0.5, (0x21, 0x91, 0x8C)),
       (0.75, (0x5E, 0xC9, 0x62)), (1.0, (0xFD, 0xE7, 0x25))]


def viridis(t):
    t = min(1, max(0, t))
    for (a, ca), (b, cb) in zip(VIR, VIR[1:]):
        if t <= b:
            k = (t - a) / (b - a)
            return "#%02X%02X%02X" % tuple(round(x + (y - x) * k) for x, y in zip(ca, cb))


def field(u, v):
    """u, v in 0..1. Diagonal gradient (as in the original) + warm spot near the front corner + gentle texture."""
    base = 0.22 + 0.46 * (u + v) / 2
    hot = 0.36 * math.exp(-(((u - 0.76) ** 2 + (v - 0.86) ** 2) / 0.05))
    tex = 0.09 * math.sin(9.0 * u + 2.3) * math.cos(8.0 * v - 1.1)
    return base + hot + tex


def cells(n):
    s = 100 / n
    out = []
    for i in range(n):
        for j in range(n):
            col = viridis(field((i + 0.5) / n, (j + 0.5) / n))
            # +0.2 overlap hides anti-aliasing seams between cells
            out.append(f'<rect x="{i*s:.3f}" y="{j*s:.3f}" width="{s+0.2:.3f}" height="{s+0.2:.3f}" fill="{col}"/>')
    return "".join(out)


def build(n, pfx, title):
    s = open(f"{D}/concept-1-strata.svg").read()
    for i in re.findall(r'id="([^"]+)"', s):
        s = s.replace(f'id="{i}"', f'id="{pfx}-{i}"').replace(f"url(#{i})", f"url(#{pfx}-{i})").replace(f'href="#{i}"', f'href="#{pfx}-{i}"')
    # exact isometric scale (concept-1 rounded 2.72/1.5704 to 2.7/1.6; this aligns faces with their edges)
    w = 2.72; h = round(272 * math.tan(math.radians(30)) / 100, 4)
    s = s.replace("matrix(2.7,1.6,-2.7,1.6,", f"matrix({w},{h},{-w},{h},")
    s = re.sub(r'<rect width="100" height="100" fill="url\(#' + pfx + r'-rast\)"/><rect width="100" height="100" fill="url\(#' + pfx + r'-hot\)"/>', cells(n), s)
    s = re.sub(r'\s*<linearGradient id="' + pfx + r'-rast".*?</linearGradient>', "", s)
    s = re.sub(r'\s*<radialGradient id="' + pfx + r'-hot".*?</radialGradient>', "", s)
    s = s.replace("<!-- bottom sheet: raster (viridis) -->", f"<!-- bottom sheet: pixelated raster, {n}x{n} viridis cells -->")
    s = re.sub(r"<title>.*?</title>", f"<title>LayeredMap - {title}</title>", s)
    return s


if __name__ == "__main__":
    nm, n32, n16 = (int(x) for x in sys.argv[1:4]) if len(sys.argv) > 3 else (8, 5, 4)
    open(f"{D}/strata-raster.svg", "w").write(build(nm, "sr", "Strata Raster"))
    open(f"{D}/strata-raster-32.svg", "w").write(build(n32, "sr32", "Strata Raster (32 px)"))
    open(f"{D}/strata-raster-16.svg", "w").write(build(n16, "sr16", "Strata Raster (16 px)"))
    print("ok", nm, n32, n16)
