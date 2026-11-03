"""iOS app icon from a design/logo-nimble master: the macOS squircle body cropped to a full-bleed
824 px square (iOS masks the corners itself), rendered at 1024 px with no transparency.
Usage (from the repo root, in the lm-logo env that has cairosvg):
  python spikes/<spike>/scripts/make-ios-icon.py design/logo-nimble/concept-8-peek.svg spikes/<spike>/ios/<App>/Assets.xcassets/AppIcon.appiconset/AppIcon.png
(dark and tinted: the same with design/logo-nimble/variants/concept-8-peek-{dark,tinted}.svg)."""
import re, sys
import cairosvg

src, out = sys.argv[1], sys.argv[2]
s = open(src).read()
sq = re.search(r'<path id="(\w+-sq)"', s).group(1)
s = s.replace('viewBox="0 0 1024 1024"', 'viewBox="100 100 824 824"', 1)
# Background: the squircle fill becomes the whole square; the clip becomes the square.
s = re.sub(rf'<use href="#{sq}" fill="([^"]+)"\s*/>', r'<rect x="100" y="100" width="824" height="824" fill="\1"/>', s, count=1)
s = s.replace(f'<use href="#{sq}"/>', '<rect x="100" y="100" width="824" height="824"/>')
# Drop the preview outline.
s = re.sub(rf'<use href="#{sq}" fill="none"[^>]*/>', '', s)
assert f'href="#{sq}"' not in s, "squircle still used"
cairosvg.svg2png(bytestring=s.encode(), write_to=out, output_width=1024, output_height=1024, background_color=None)
