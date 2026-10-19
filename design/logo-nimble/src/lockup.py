"""Wordmark lockups and web-font subsets.

1. Subsets the repo's bundled OFL fonts (assets/fonts/Inter-SemiBold.ttf and
   NotoSansSC-Medium.otf) to only the glyphs the comparison page needs, as WOFF2
   in fonts/ (embedded into index.html as base64 by page.py, so it works offline).
2. Writes lockups/concept-N-<name>-lockup-{light,dark}.svg: icon + "NimbleMap"
   + "千层图", with text converted to outlines so CairoSVG renders it exactly and
   the files need no installed font. Kerning from the GPOS table is not applied
   (the Inter pairs in "NimbleMap" are negligible); letter-spacing -0.015 em is.

Usage:  mamba run -n lm-logo-fonts python src/lockup.py   (needs fonttools + brotli)
"""
import glob, os, re
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools import subset

D = "/Volumes/LaCie/repos/LayeredMap/design/logo-nimble"
FONTS = "/Volumes/LaCie/repos/LayeredMap/assets/fonts"
INTER = f"{FONTS}/Inter-SemiBold.ttf"
NOTO = f"{FONTS}/NotoSansSC-Medium.otf"
os.makedirs(f"{D}/fonts", exist_ok=True)
os.makedirs(f"{D}/lockups", exist_ok=True)


def make_subset(src, text, out):
    opt = subset.Options()
    opt.flavor = "woff2"
    opt.layout_features = ["kern", "liga", "calt"]
    opt.name_IDs = ["*"]  # keep copyright / licence strings (OFL)
    f = TTFont(src)
    s = subset.Subsetter(opt)
    s.populate(text=text)
    s.subset(f)
    f.flavor = "woff2"
    f.save(out)
    return os.path.getsize(out)


print("inter", make_subset(INTER, "NimbleMap", f"{D}/fonts/Inter-SemiBold-NimbleMap.woff2"))
print("noto", make_subset(NOTO, "千层图", f"{D}/fonts/NotoSansSC-Medium-QianCengTu.woff2"))


def outline(fontpath, text, size, x, baseline, tracking=0.0):
    """Return (svg path d, advance) for text set at `size` px starting at x."""
    f = TTFont(fontpath)
    gs = f.getGlyphSet()
    cmap = f.getBestCmap()
    upm = f["head"].unitsPerEm
    k = size / upm
    d, cx = "", x
    for ch in text:
        g = cmap[ord(ch)]
        pen = SVGPathPen(gs, ntos=lambda v: f"{v:.1f}".rstrip("0").rstrip("."))
        gs[g].draw(TransformPen(pen, (k, 0, 0, -k, cx, baseline)))
        d += pen.getCommands()
        cx += gs[g].width * k + tracking * size
    return d, cx - tracking * size - x


def icon_body(svgpath):
    s = open(svgpath).read()
    s = re.sub(r"<title>.*?</title>", "", s)
    return re.sub(r"^.*?<svg[^>]*>|</svg>\s*$", "", s, flags=re.S)


H = 400
mains = sorted(p for p in glob.glob(f"{D}/concept-*.svg") if not p.endswith(("-32.svg", "-16.svg")))
for theme in ("light", "dark"):
    ink, sub, bg = ("#1D1D1F", "#6E6E73", "#FFFFFF") if theme == "light" else ("#F5F5F7", "#A1A1A6", "#1C1C1E")
    word, ww = outline(INTER, "NimbleMap", 150, 0, 0, -0.015)
    zh, zw = outline(NOTO, "千层图", 116, 0, 0)
    for p in mains:
        base = os.path.basename(p)[:-4]
        src = p if theme == "light" else f"{D}/variants/{base}-dark.svg"
        body = icon_body(src)
        ic = 300  # icon box; the squircle is 824/1024 of it
        x_word = 40 + ic + 20
        x_div = x_word + ww + 56
        x_zh = x_div + 56
        W = int(x_zh + zw + 60)
        svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">
  <title>NimbleMap lockup - {base} ({theme})</title>
  <rect width="{W}" height="{H}" fill="{bg}"/>
  <g transform="translate(40 50) scale({ic/1024})">{body}</g>
  <path transform="translate({x_word:.1f} 254)" d="{word}" fill="{ink}"/>
  <rect x="{x_div:.1f}" y="130" width="4" height="140" rx="2" fill="{sub}" fill-opacity="0.45"/>
  <path transform="translate({x_zh:.1f} 247)" d="{zh}" fill="{sub}"/>
</svg>
'''
        open(f"{D}/lockups/{base}-lockup-{theme}.svg", "w").write(svg)
print("lockups", len(mains) * 2)
