"""Build design/logo-nimble/index.html: a self-contained, offline comparison page.

All SVGs are inlined (ids re-prefixed per instance); the wordmark fonts are the
WOFF2 subsets written by src/lockup.py, embedded as base64. No network requests.

Usage:  python3 src/page.py
"""
import base64, itertools, re

D = "/Volumes/LaCie/repos/LayeredMap/design/logo-nimble"
OLD = "/Volumes/LaCie/repos/LayeredMap/design/logo"

INFO = [
    ("1", "dart", "Dart", "Motion",
     "A paper dart folded from a map sheet (a river and a park printed on the wing) at the end of a dotted route, flying over a coast.",
     "Says “quick” at first glance; the printed wing, the dotted route and the coastline keep it a map. Light, airy palette.",
     "Paper planes are a crowded motif. It is pale on light backgrounds at 16 px, and the dark variant is the stronger one.",
     "Medium-high: Telegram (white paper plane on blue) and the generic “send” glyph; also near Amap’s white navigation-arrow mark. Keeping the map print and the light sky helps, but the silhouette is shared."),
    ("2", "feather", "Feather", "Lightness",
     "A feather whose two vanes are sea and land; the shaft is the coastline, and a coral dot marks a place.",
     "The lightest-feeling mark, and the warm paper background stands out in a Dock full of blue. A clean diagonal blade at 16 px.",
     "Reads as a leaf or quill before it reads as a map, so the map story depends on the blue/green colour convention. A quill also suggests a writing app.",
     "Low: no listed logo uses a feather. Generic overlap with note-taking and eco brands."),
    ("3", "lift", "Lift", "Lightness",
     "One isometric map sheet lifting off the ground, its shadow left behind, with a pin standing on it.",
     "Continuity with the shipped Strata (same 30° sheet) while dropping the “many layers” story. Very legible as a map.",
     "Pin-on-map-sheet is the most generic map-app picture; “nimble” is carried only by the gap to the shadow. Weak at 16 px.",
     "Medium (generic, not one brand). Note: Strata’s stacked sheets next to the word “Nimble” come close to HPE Nimble Storage’s green stacked layers; Lift’s single sheet avoids that."),
    ("4", "hover", "Hover", "Creature",
     "A geometric hummingbird hovering over a quiet map, its beak pointing at one spot on the map.",
     "Tells the product story in one image: quick, precise and curious, “click anything to see where it stands”. A creature is rare among map apps, and the silhouette survives at 32 and 16 px.",
     "The most illustrative of the eight, so it needs a professional drawing pass (wing and tail curves) before shipping. The map is the secondary layer.",
     "Low-medium: bird marks are common, but Twitter’s bird is round and rising, and Apple’s Swift is a swift on orange. A hummingbird in blue on a pale map is distinct from both."),
    ("5", "hop", "Hop", "Quick pin",
     "A coral pin leaning into a hop: a dotted arc from the last place, about to land in a ring.",
     "Unmistakably a map and unmistakably in motion; friendly and simple; strong at every size.",
     "The pin is the most common map glyph, so this risks looking like a delivery or check-in app.",
     "Medium: generic pins (Google Maps, Baidu Maps, Amap all build on pins). I deliberately avoided ghosted copies behind the pin, which is Nimble CRM’s yellow motion trail."),
    ("6", "wings", "Wings", "Folded map",
     "A two-panel paper map opened like wings and banking like a glider: sea on one panel, streets on the other, one route across the fold.",
     "The strongest Dock presence (accent-blue tile) and an instant “map” read; the open fold adds lift and motion. Clear in dark and tinted modes.",
     "Can read as an open book or e-reader icon. Folded maps are a familiar maps metaphor.",
     "Low-medium: generic folded-map icons. No close match among the listed brands."),
    ("7", "route-n", "Route N", "Abstract N · v2 · owner’s #3",
     "v2: a real map first: land, a lake, a hill with layered contour tints and a street grid. The N is a major road in that map with a teal route drawn along it, from a navy start pin to a coral arrival dot.",
     "Now reads as “a map with a route” before “N”: the lake, the hill and the streets carry as much weight as the letter. Still the boldest mark at 16 px, where the road itself turns teal.",
     "Still a Latin initial (weak for 千层图). The hill is clipped by the top-left corner. Busier than v1.",
     "Low-medium: green map tiles (AllTrails, QGIS) and route-on-map icons. The teal route keeps it away from Apple Maps’ blue route. Not close to HPE Nimble Storage or Nimble CRM."),
    ("8", "peek", "Peek", "Lens · v2 · chosen",
     "v2 (chosen): a magnifier, ring and handle in one solid navy, over a calm street map. Inside is a stepped viridis raster patch and the coral clicked feature, off-centre.",
     "Says “look closer at your data”, and the handle makes it unmistakably a magnifier rather than a rifle scope (no crosshair). Hand-drawn 32 and 16 px art keeps the ring, the handle, a two-colour patch and the coral feature. A website version for 30 px is included.",
     "Lenses usually mean search, so the raster and coral feature have to carry “map data”. The outside map is quiet by design and mostly disappears below 64 px.",
     "Low for the listed brands; generic overlap with search icons. Not a scope: no crosshair, and it has a handle."),
    ("9", "riffle", "Riffle", "Nimble + layers · v2 · owner’s #2",
     "v2: three big thin map sheets (raster, polygons, streets with the coral place) fanned from the bottom-left corner so together they fill the squircle.",
     "Carries both names, “nimble” in the flick and “千层图” in the thin layers. The front map is now large and legible, with a sea, a park, streets and a dotted route. The back sheets show as clear raster and polygon wedges.",
     "The front sheet dominates, and at 16 px the two back sheets merge into one coloured edge. It can still read as a stack of cards.",
     "Low-medium: card-stack icons. Not Nimble Storage: the sheets are flat, fanned and different, not stacked green isometric plates."),
]
FIT = {
    "1": ("Weak", "One folded sheet; nothing about layers."),
    "2": ("Medium potential, weak as drawn", "Plumage is naturally many thin overlapping layers. A redraw as two or three stacked translucent vanes would bridge both names, but the current two-vane drawing does not."),
    "3": ("Medium", "One sheet lifting off implies a stack below and keeps the Strata heritage. Adding visible sheets beneath would drift toward Nimble Storage’s green stacked isometric layers, so keep it to one."),
    "4": ("Weak", "The map is a single plane under the bird; the story is nimble, not layers."),
    "5": ("Weak", "A pin and a ground plane; no layers."),
    "6": ("Medium-good", "Two panels of different content are already two sheets; a third accordion panel would make it a light, flying stack of map layers."),
    "7": ("None", "A Latin initial; it fits NimbleMap only, and 千层图 not at all."),
    "8": ("Good", "The lens shows a second data layer under the street map: looking through the layers, which is what 千层图 promises."),
    "9": ("Strong", "Designed for this: a few thin layers, in motion."),
}
cnt = itertools.count()


def inline_file(path, size, label):
    s = open(path).read()
    p = f"i{next(cnt)}_"
    for i in sorted(set(re.findall(r'id="([^"]+)"', s)), key=len, reverse=True):
        s = s.replace(f'id="{i}"', f'id="{p}{i}"').replace(f"url(#{i})", f"url(#{p}{i})").replace(f'href="#{i}"', f'href="#{p}{i}"')
    s = re.sub(r"<title>.*?</title>", "", s, flags=re.S)
    s = re.sub(r"<!--.*?-->", "", s, flags=re.S)
    s = s.replace('width="1024" height="1024"', f'width="{size}" height="{size}" role="img" aria-label="{label}"', 1)
    return re.sub(r">\s+<", "><", s).strip()


def ic(n, name, size, var=""):
    base = f"concept-{n}-{name}"
    if var in ("dark", "tinted"):
        path = f"{D}/variants/{base}-{var}.svg"
    elif size in (32, 16):
        path = f"{D}/{base}-{size}.svg"
    else:
        path = f"{D}/{base}.svg"
    return inline_file(path, size, f"{name} {var}".strip())


def strata(size):
    path = f"{OLD}/strata-raster-{size}.svg" if size in (32, 16) else f"{OLD}/strata-raster.svg"
    return inline_file(path, size, "Strata (current)")


def b64(path):
    return base64.b64encode(open(path, "rb").read()).decode()


INTER = b64(f"{D}/fonts/Inter-SemiBold-NimbleMap.woff2")
NOTO = b64(f"{D}/fonts/NotoSansSC-Medium-QianCengTu.woff2")


def sizes(fn):
    return "".join(f"<figure>{fn(z)}<figcaption>{z}</figcaption></figure>" for z in (128, 64, 32, 16))


def v1(n, name, size):
    suf = f"-{size}" if size in (32, 16) else ""
    return inline_file(f"{D}/v1/concept-{n}-{name}-v1{suf}.svg", size, f"{name} v1")


def v1row(n, name):
    if n not in ("7", "8", "9"):
        return ""
    a = "".join(f"<figure>{v1(n, name, z)}<figcaption>v1 {z}</figcaption></figure>" for z in (96, 32, 16))
    b = "".join(f"<figure>{ic(n, name, z)}<figcaption>v2 {z}</figcaption></figure>" for z in (96, 32, 16))
    return f'<div class="sizes light vcmp">{a}<span class="arrow">→</span>{b}</div>'


def siterow(n):
    if n != "8":
        return ""
    site = lambda z: inline_file(f"{D}/peek-site.svg", z, "Peek website mark")
    return (f'<div class="site"><div class="hdr light">{site(30)}<span class="wm">NimbleMap</span><span class="zh">千层图</span></div>'
            f'<div class="hdr dark">{site(30)}<span class="wm">NimbleMap</span><span class="zh">千层图</span></div></div>'
            '<p class="legend">Website header at 30 px: <code style="margin:0">peek-site.svg</code> (replaces <code style="margin:0">web/lm/assets/logo.svg</code>).</p>')


cards = []
for n, name, title, direction, idea, good, weak, risk in INFO:
    cards.append(f'''
<section class="card" id="c{n}">
  <header><span class="num">{n}</span><h2>{title}</h2><span class="tag{' chosen' if n == '8' else ''}">{direction}</span><code>concept-{n}-{name}.svg</code></header>
  <div class="hero">
    <div class="pane light">{ic(n, name, 220)}</div>
    <div class="pane dark">{ic(n, name, 220)}</div>
  </div>
  <div class="sizes light">{sizes(lambda z: ic(n, name, z))}</div>
  <div class="sizes dark">{sizes(lambda z: ic(n, name, z))}</div>
  <div class="appear dark">
    <figure>{ic(n, name, 72)}<figcaption>Default</figcaption></figure>
    <figure>{ic(n, name, 72, "dark")}<figcaption>Dark</figcaption></figure>
    <figure>{ic(n, name, 72, "tinted")}<figcaption>Tinted</figcaption></figure>
  </div>
  <div class="lockup light">{ic(n, name, 52)}<span class="wm">NimbleMap</span><span class="zh">千层图</span></div>
  <div class="lockup dark">{ic(n, name, 52, "dark")}<span class="wm">NimbleMap</span><span class="zh">千层图</span></div>
  {v1row(n, name)}{siterow(n)}
  <dl class="text">
    <dt>Idea</dt><dd>{idea}</dd>
    <dt>Strengths</dt><dd>{good}</dd>
    <dt>Weaknesses</dt><dd>{weak}</dd>
    <dt>Resembles</dt><dd>{risk}</dd>
    <dt>千层图 fit</dt><dd><b class="fit">{FIT[n][0]}.</b> {FIT[n][1]}</dd>
  </dl>
</section>''')

ref = f'''
<section class="card ref" id="strata">
  <header><span class="num">Now</span><h2>Strata (current icon)</h2><span class="tag">Reference</span><code>design/logo/strata-raster.svg</code></header>
  <div class="hero">
    <div class="pane light">{strata(220)}</div>
    <div class="pane dark">{strata(220)}</div>
  </div>
  <div class="sizes light">{sizes(strata)}</div>
  <div class="sizes dark">{sizes(strata)}</div>
  <div class="lockup light">{strata(52)}<span class="wm">NimbleMap</span><span class="zh">千层图</span></div>
  <dl class="text"><dt>Note</dt><dd>Made for “layered”. It literally fits 千层图 (stacked layers), but paired with the word “Nimble”, its stacked sheets with a green base come close to HPE Nimble Storage’s mark (green stacked isometric layers beside the word “nimble”). If the layered idea is kept, Riffle (flat, fanned, in motion) carries it without that resemblance.</dd></dl>
</section>'''


def dock(theme):
    items = "".join(f'<div class="dock-item" title="{n} {t}">{ic(n, nm, 64, "dark" if theme == "dark" else "")}<i></i></div>' for n, nm, t, *_ in INFO)
    items += f'<div class="sep"></div><div class="dock-item" title="Strata (current)">{strata(64)}<i></i></div>'
    return f'<div class="dock-wrap {theme}"><div class="dock"><div class="ghost"></div><div class="ghost"></div>{items}</div></div>'


html = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>NimbleMap Icon Concepts</title>
<style>
@font-face {{ font-family: "NM Inter"; font-weight: 600; src: url(data:font/woff2;base64,{INTER}) format("woff2"); }}
@font-face {{ font-family: "NM Noto SC"; font-weight: 500; src: url(data:font/woff2;base64,{NOTO}) format("woff2"); }}
:root {{
  --bg: #F5F5F7; --surface: #FFFFFF; --text: #1D1D1F; --muted: #6E6E73; --line: #E3E3E8; --accent: #2563EB;
  --pane-light: #F5F5F7; --pane-dark: #1C1C1E; --ink-light: #1D1D1F; --ink-dark: #F5F5F7;
  --sans: -apple-system, BlinkMacSystemFont, "Helvetica Neue", "PingFang SC", "Noto Sans SC", "Microsoft YaHei", sans-serif;
}}
@media (prefers-color-scheme: dark) {{
  :root:not([data-theme="light"]) {{ --bg: #000000; --surface: #161618; --text: #F5F5F7; --muted: #A1A1A6; --line: #2C2C2E; --accent: #6E9BFF; color-scheme: dark; }}
}}
:root[data-theme="dark"] {{ --bg: #000000; --surface: #161618; --text: #F5F5F7; --muted: #A1A1A6; --line: #2C2C2E; --accent: #6E9BFF; color-scheme: dark; }}
* {{ box-sizing: border-box; }}
html {{ -webkit-text-size-adjust: 100%; }}
body {{ margin: 0; background: var(--bg); color: var(--text); font: 15px/1.55 var(--sans); }}
main {{ max-width: 1160px; margin: 0 auto; padding: 48px 16px 80px; }}
h1 {{ font-weight: 650; font-size: clamp(28px, 5vw, 40px); letter-spacing: -0.02em; margin: 0 0 8px; }}
.lede {{ color: var(--muted); max-width: 70ch; margin: 0 0 8px; }}
h2 {{ font-weight: 650; font-size: 21px; letter-spacing: -0.01em; margin: 0; }}
h3 {{ font-weight: 600; font-size: 13px; text-transform: uppercase; letter-spacing: .06em; color: var(--muted); margin: 40px 0 12px; }}
svg {{ display: block; height: auto; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 520px), 1fr)); gap: 24px; }}
.card {{ background: var(--surface); border: 1px solid var(--line); border-radius: 20px; padding: 20px; overflow: hidden; display: grid; gap: 8px; align-content: start; }}
.card.ref {{ border-style: dashed; }}
.card header {{ display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; margin-bottom: 8px; }}
.num {{ font-weight: 650; color: var(--accent); }}
.tag.chosen {{ background: var(--accent); color: #fff; border-color: transparent; }}
.vcmp {{ align-items: center; }}
.arrow {{ font-size: 20px; opacity: .5; }}
.site {{ display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 8px; }}
.hdr {{ display: flex; align-items: center; gap: 8px; padding: 10px 14px; border-radius: 12px; min-width: 0; overflow: hidden; }}
.hdr svg {{ width: 30px; flex: none; }}
.hdr .wm {{ font-size: 17px; }} .hdr .zh {{ font-size: 15px; padding-left: 8px; }}
.tag {{ font-size: 12px; color: var(--muted); border: 1px solid var(--line); border-radius: 99px; padding: 1px 8px; }}
code {{ font: 12px ui-monospace, SFMono-Regular, Menlo, monospace; color: var(--muted); margin-left: auto; }}
.light {{ background: var(--pane-light); color: var(--ink-light); }}
.dark {{ background: var(--pane-dark); color: var(--ink-dark); }}
.hero {{ display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); border-radius: 14px; overflow: hidden; }}
.pane {{ display: grid; place-items: center; padding: 16px; }}
.pane svg {{ width: 100%; max-width: 220px; height: auto; }}
.card, .grid > * {{ min-width: 0; }}
.sizes, .appear {{ display: flex; align-items: flex-end; gap: 18px; padding: 14px 16px; border-radius: 12px; flex-wrap: wrap; }}
figure {{ margin: 0; display: grid; justify-items: center; gap: 4px; }}
figcaption {{ font-size: 11px; opacity: .6; }}
.appear {{ justify-content: flex-start; }}
.lockup {{ display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-radius: 12px; min-width: 0; flex-wrap: wrap; }}
.lockup svg {{ width: 52px; flex: none; }}
.wm {{ font-family: "NM Inter", Inter, var(--sans); font-weight: 600; font-size: clamp(22px, 4vw, 30px); letter-spacing: -0.015em; white-space: nowrap; }}
.zh {{ font-family: "NM Noto SC", "PingFang SC", "Noto Sans SC", var(--sans); font-weight: 500; font-size: clamp(20px, 3.6vw, 26px); opacity: .6; padding-left: 12px; border-left: 2px solid currentColor; border-left-color: color-mix(in srgb, currentColor 30%, transparent); line-height: 1.1; }}
dl.text {{ margin: 8px 0 0; display: grid; grid-template-columns: max-content 1fr; gap: 6px 14px; }}
dt {{ font-weight: 600; font-size: 13px; color: var(--text); }}
dd {{ margin: 0; color: var(--muted); }}
.fit {{ color: var(--text); font-weight: 600; }}
.dock-wrap {{ border-radius: 20px; padding: 56px 12px 16px; overflow-x: auto; }}
.dock-wrap.light {{ background: linear-gradient(160deg, #C9D8EE, #E8DCCF); color: #1D1D1F; }}
.dock-wrap.dark {{ background: linear-gradient(160deg, #0F1A2E, #2A2233); color: #F5F5F7; margin-top: 12px; }}
.dock {{ display: flex; gap: 8px; width: max-content; margin: 0 auto; padding: 8px 10px 6px; border-radius: 22px; align-items: flex-start;
  background: rgba(255,255,255,.38); border: 1px solid rgba(255,255,255,.5); }}
.dark .dock {{ background: rgba(40,40,46,.6); border-color: rgba(255,255,255,.12); }}
.dock-item, .ghost {{ width: 64px; display: grid; justify-items: center; gap: 3px; }}
.dock-item svg {{ width: 64px; }}
.dock-item i {{ width: 4px; height: 4px; border-radius: 50%; background: currentColor; opacity: .55; }}
.ghost {{ height: 64px; border-radius: 15px; background: rgba(127,127,127,.25); transform: scale(.82); }}
.sep {{ width: 1px; align-self: stretch; margin: 4px 2px 10px; background: currentColor; opacity: .25; }}
.legend {{ font-size: 13px; color: var(--muted); margin-top: 8px; }}
.rec {{ background: var(--surface); border: 1px solid var(--line); border-radius: 16px; padding: 16px 20px; max-width: 80ch; }}
.rec p {{ margin: 6px 0; }}
footer {{ margin-top: 48px; color: var(--muted); font-size: 13px; }}
@media (max-width: 560px) {{
  main {{ padding-top: 32px; }}
  .card {{ padding: 14px; border-radius: 16px; }}
  .sizes, .appear {{ gap: 12px; }}
  .sizes figure:first-child svg {{ width: 96px; height: 96px; }}
  dl.text {{ grid-template-columns: 1fr; }}
  dt {{ margin-top: 6px; }}
  .dock-item, .ghost, .dock-item svg {{ width: 44px; }} .ghost {{ height: 44px; }}
}}
</style>
</head>
<body>
<main>
  <h1>NimbleMap icon concepts</h1>
  <p class="lede">Nine directions (three revised to v2) for renaming LayeredMap to NimbleMap / 千层图 (“thousand-layer map”, after 千层酥, mille-feuille): the nimble alternative to QGIS. Each icon has to feel light, quick and curious and still say <em>map</em>; the “千层图 fit” line says how well it also carries the Chinese name’s idea of layers. Each one is shown large, at 128/64/32/16 px (the 32 and 16 px sizes use hand-simplified artwork), on light and dark, in its dark and tinted appearances, in a Dock, and beside the wordmark. The current Strata icon is included for reference.</p>
  <div class="rec">
    <p><b>Chosen: 8 Peek.</b> Owner’s ranking: 1 Peek, 2 Riffle, 3 Route N. Peek v2 is polished for production, with hand-drawn 32 and 16 px artwork and a 30 px website mark. Riffle and Route N have v2 revisions from the owner’s notes; the v1 → v2 row in each card compares them.</p>
  </div>

  <h3>In the Dock</h3>
  {dock("light")}
  {dock("dark")}
  <p class="legend">Left to right: concepts 1–9, then the current Strata icon after the divider. The dark Dock uses each concept’s dark appearance.</p>

  <h3>Concepts</h3>
  <div class="grid">{"".join(cards)}{ref}</div>

  <footer>Sources: <code style="margin:0">design/logo-nimble/</code> (SVGs, plus <code style="margin:0">-32</code>/<code style="margin:0">-16</code> small artwork), <code style="margin:0">variants/</code> (dark, tinted), <code style="margin:0">lockups/</code>, and <code style="margin:0">png/</code>. The wordmark is Inter SemiBold and the Chinese is Noto Sans SC Medium, both OFL, subset from the repo’s <code style="margin:0">assets/fonts</code> and embedded, so this page works offline. See README.md.</footer>
</main>
</body>
</html>
'''
open(f"{D}/index.html", "w").write(html)
print(len(html))
