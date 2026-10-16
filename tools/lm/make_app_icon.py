"""Builds the app icon from the chosen logo SVG: packaging/lm/icon/AppIcon.iconset (every macOS
size), packaging/lm/AppIcon.icns (via iconutil), and assets/icon/icon-256.png for the window icon.

Usage: mamba run -n lm-logo python tools/lm/make_app_icon.py design/logo/concept-1-strata.svg
"""
import pathlib, subprocess, sys
import cairosvg

svg = pathlib.Path(sys.argv[1]).read_bytes()
iconset = pathlib.Path("packaging/lm/icon/AppIcon.iconset")
iconset.mkdir(parents=True, exist_ok=True)
for base in (16, 32, 128, 256, 512):
    for scale in (1, 2):
        px = base * scale
        name = f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
        cairosvg.svg2png(bytestring=svg, write_to=str(iconset / name), output_width=px, output_height=px)
subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", "packaging/lm/AppIcon.icns"], check=True)
out = pathlib.Path("assets/icon"); out.mkdir(parents=True, exist_ok=True)
cairosvg.svg2png(bytestring=svg, write_to=str(out / "icon-256.png"), output_width=256, output_height=256)
print("ok")
