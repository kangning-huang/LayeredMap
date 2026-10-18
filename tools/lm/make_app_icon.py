"""Builds the app icon from the chosen logo: packaging/lm/icon/AppIcon.iconset (every macOS size),
packaging/lm/AppIcon.icns (via iconutil), packaging/lm/windows/layeredmap.ico (WP6.2: the sizes
Windows asks for at 100-200 % scaling and in Explorer's views, as PNG entries) and
assets/icon/icon-256.png (the window icon and the header icon). Small sizes use the logo's simplified artwork when it exists (`<name>-32.svg` for
32 px and below 64, `<name>-16.svg` for 16 px), as the logo README recommends.

Usage: mamba run -n lm-logo python tools/lm/make_app_icon.py design/logo/strata-raster.svg
(the owner's choice, 2026-10-16: Strata with the pixelated raster layer)
"""
import pathlib, struct, subprocess, sys
import cairosvg

main = pathlib.Path(sys.argv[1])
def art(px: int) -> bytes:
    for size, limit in ((16, 16), (32, 63)):
        small = main.with_name(f"{main.stem}-{size}.svg")
        if px <= limit and small.exists():
            return small.read_bytes()
    return main.read_bytes()

iconset = pathlib.Path("packaging/lm/icon/AppIcon.iconset")
iconset.mkdir(parents=True, exist_ok=True)
for base in (16, 32, 128, 256, 512):
    for scale in (1, 2):
        px = base * scale
        name = f"icon_{base}x{base}{'@2x' if scale == 2 else ''}.png"
        cairosvg.svg2png(bytestring=art(px), write_to=str(iconset / name), output_width=px, output_height=px)
subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", "packaging/lm/AppIcon.icns"], check=True)
# Windows: 16/20/24/32 px are the small icon at 100/125/150/200 %, 32/40/48/64 the large one, 256
# Explorer's extra-large view. PNG entries (read by Windows Vista and later), largest last.
ico_sizes = (16, 20, 24, 32, 40, 48, 64, 256)
pngs = [cairosvg.svg2png(bytestring=art(px), output_width=px, output_height=px) for px in ico_sizes]
header = struct.pack("<HHH", 0, 1, len(pngs))
offset = len(header) + 16 * len(pngs)
entries, data = b"", b""
for px, png in zip(ico_sizes, pngs):
    side = 0 if px >= 256 else px  # 0 means 256
    entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(png), offset + len(data))
    data += png
ico = pathlib.Path("packaging/lm/windows/layeredmap.ico"); ico.parent.mkdir(parents=True, exist_ok=True)
ico.write_bytes(header + entries + data)
out = pathlib.Path("assets/icon"); out.mkdir(parents=True, exist_ok=True)
cairosvg.svg2png(bytestring=art(256), write_to=str(out / "icon-256.png"), output_width=256, output_height=256)
print("ok")
