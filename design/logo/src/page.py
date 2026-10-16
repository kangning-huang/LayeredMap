import re, itertools, os

D = "/Volumes/LaCie/repos/LayeredMap/design/logo"
INFO = [
    ("1", "strata", "Strata",
     "Three isometric map sheets float in a stack: a viridis raster at the bottom, district polygons in the middle, points on top.",
     "It is the literal app model: each sheet is one layer, and because they share one footprint the eye reads them as the same place seen through different data.",
     "Stacked-sheets is a familiar “layers” glyph, so it is less ownable; the relationship between layers is implied by alignment rather than shown."),
    ("2", "overlay", "Overlay",
     "A circular buffer and a district polygon overlap on a faint graticule; only their intersection turns the accent blue, with one feature inside it.",
     "It shows the spatial relationship itself, the overlap of two layers, rather than the layers as objects, and it is the simplest mark at 16 px.",
     "It reads as an abstract Venn diagram; the “map” cue rests on the graticule and the polygon outline, which disappear at small sizes."),
    ("3", "fold", "Fold",
     "A tri-fold paper map whose panels are different layers: contours, districts and points, joined by one continuous route.",
     "The route crossing every panel says that the layers share one geography; the fold says “map” instantly.",
     "The busiest option, and it turns to mush at 16 px. The folded map is also a common maps metaphor, closer to other map apps’ territory."),
    ("4", "strata-pin", "Strata Pin",
     "A location pin sliced into three strata (points, polygons, raster colours) standing on a quiet street grid.",
     "One place, many layers: the pin anchors the idea of location and the cut bands say the place is made of stacked data.",
     "Pins are the most common map glyph, so it risks looking like a generic navigation or check-in app; the strata are decorative more than spatial."),
    ("5", "swipe", "Swipe",
     "One map split by a swipe divider: the left half is a neutral base map, the right half the same districts and river as a choropleth.",
     "The river and boundaries run straight through the divider, so it shows that two layers describe the same ground, the comparison researchers actually do.",
     "It describes a tool (swipe-compare) rather than the whole app, and it can read as a before/after photo editor icon."),
    ("6", "peel", "Peel",
     "A blue base-map sheet with its corner peeled back, revealing a raster layer underneath.",
     "Lifting one layer to see the next is a direct gesture for “there is more data under this map”; strong silhouette and colour block at every size.",
     "Page-curl is a skeuomorphic trope (“document”, “next page”); the map content on the top sheet is subtle and the relationship between layers is hidden rather than shown."),
]
counter = itertools.count()


def inline(n, name, size, label=True):
    s = open(f"{D}/concept-{n}-{name}.svg").read()
    p = f"c{next(counter)}_"
    ids = re.findall(r'id="([^"]+)"', s)
    for i in ids:
        s = s.replace(f'id="{i}"', f'id="{p}{i}"').replace(f'url(#{i})', f'url(#{p}{i})').replace(f'href="#{i}"', f'href="#{p}{i}"')
    s = re.sub(r'<title>.*?</title>', '', s)
    s = s.replace('width="1024" height="1024"', f'width="{size}" height="{size}" role="img" aria-label="Concept {n}"', 1)
    return s.replace("\n", "")


cards = []
for n, name, title, idea, rel, weak in INFO:
    sizes_l = "".join(f'<figure>{inline(n,name,z)}<figcaption>{z}</figcaption></figure>' for z in (128, 64, 32, 16))
    sizes_d = "".join(f'<figure>{inline(n,name,z)}<figcaption>{z}</figcaption></figure>' for z in (128, 64, 32, 16))
    cards.append(f'''
<section class="card" id="c{n}">
  <header><span class="num">{n}</span><h2>{title}</h2><code>concept-{n}-{name}.svg</code></header>
  <div class="hero">
    <div class="pane light">{inline(n,name,256)}</div>
    <div class="pane dark">{inline(n,name,256)}</div>
  </div>
  <div class="sizes light">{sizes_l}</div>
  <div class="sizes dark">{sizes_d}</div>
  <div class="lockups">
    <div class="lockup light">{inline(n,name,56)}<span>LayeredMap</span></div>
    <div class="lockup dark">{inline(n,name,56)}<span>LayeredMap</span></div>
  </div>
  <div class="text">
    <p><b>Idea.</b> {idea}</p>
    <p><b>Layers + spatial relationship.</b> {rel}</p>
    <p><b>Weakness.</b> {weak}</p>
  </div>
</section>''')

FILLER = '<div class="ghost"></div>'


def dock(theme):
    icons = "".join(f'<div class="dock-item" title="Concept {n} – {t}">{inline(n,nm,64)}<i></i></div>' for n, nm, t, *_ in INFO)
    return f'<div class="dock-wrap {theme}"><div class="dock">{FILLER}{FILLER}{icons}{FILLER}</div></div>'


html = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>LayeredMap Logo Concepts</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap" rel="stylesheet">
<style>
:root {{
  --bg: #F5F5F7; --surface: #FFFFFF; --text: #1D1D1F; --muted: #6E6E73; --line: #E3E3E8;
  --accent: #2563EB; --pane-light: #F5F5F7; --pane-dark: #1C1C1E; --ink-light: #1D1D1F; --ink-dark: #F5F5F7;
}}
@media (prefers-color-scheme: dark) {{
  :root:not([data-theme="light"]) {{ --bg: #000000; --surface: #161618; --text: #F5F5F7; --muted: #A1A1A6; --line: #2C2C2E; }}
}}
:root[data-theme="dark"] {{ --bg: #000000; --surface: #161618; --text: #F5F5F7; --muted: #A1A1A6; --line: #2C2C2E; }}
* {{ box-sizing: border-box; }}
html {{ -webkit-text-size-adjust: 100%; }}
body {{ margin: 0; background: var(--bg); color: var(--text); font: 15px/1.55 Inter, -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; }}
main {{ max-width: 1120px; margin: 0 auto; padding: 48px 16px 80px; }}
h1 {{ font-weight: 600; font-size: clamp(28px, 5vw, 40px); letter-spacing: -0.02em; margin: 0 0 8px; }}
.lede {{ color: var(--muted); max-width: 680px; margin: 0 0 32px; }}
h2 {{ font-weight: 600; font-size: 22px; letter-spacing: -0.01em; margin: 0; }}
h3 {{ font-weight: 600; font-size: 13px; text-transform: uppercase; letter-spacing: .06em; color: var(--muted); margin: 40px 0 12px; }}
svg {{ display: block; max-width: 100%; height: auto; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 520px), 1fr)); gap: 24px; }}
.card {{ background: var(--surface); border: 1px solid var(--line); border-radius: 20px; padding: 20px; overflow: hidden; }}
.card header {{ display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; margin-bottom: 16px; }}
.num {{ font-weight: 600; color: var(--accent); }}
code {{ font: 12px ui-monospace, SFMono-Regular, Menlo, monospace; color: var(--muted); margin-left: auto; }}
.light {{ background: var(--pane-light); color: var(--ink-light); }}
.dark {{ background: var(--pane-dark); color: var(--ink-dark); }}
.hero {{ display: grid; grid-template-columns: 1fr 1fr; border-radius: 14px; overflow: hidden; }}
.pane {{ display: grid; place-items: center; padding: 16px; }}
.pane svg {{ width: 100%; max-width: 256px; }}
.sizes {{ display: flex; align-items: flex-end; gap: 18px; padding: 14px 16px; margin-top: 8px; border-radius: 12px; flex-wrap: wrap; }}
.sizes figure {{ margin: 0; display: grid; justify-items: center; gap: 4px; }}
.sizes figcaption {{ font-size: 11px; opacity: .55; }}
.sizes svg {{ max-width: none; }}
.lockups {{ display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin-top: 8px; }}
.lockup {{ display: flex; align-items: center; gap: 8px; padding: 14px 16px; border-radius: 12px; min-width: 0; }}
.lockup svg {{ width: 56px; flex: none; }}
.lockup span {{ font-weight: 600; font-size: clamp(18px, 3.4vw, 24px); letter-spacing: -0.015em; white-space: nowrap; }}
.text p {{ margin: 14px 0 0; color: var(--muted); }}
.text b {{ color: var(--text); font-weight: 600; }}
.dock-wrap {{ border-radius: 20px; padding: 56px 12px 16px; overflow-x: auto; }}
.dock-wrap.light {{ background: linear-gradient(160deg, #C9D8EE, #E8DCCF); }}
.dock-wrap.dark {{ background: linear-gradient(160deg, #0F1A2E, #2A2233); margin-top: 12px; }}
.dock {{ display: flex; gap: 8px; width: max-content; margin: 0 auto; padding: 8px 10px 6px; border-radius: 22px;
  background: rgba(255,255,255,.38); border: 1px solid rgba(255,255,255,.5); backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px); }}
.dark .dock {{ background: rgba(40,40,46,.55); border-color: rgba(255,255,255,.12); }}
.dock-item, .ghost {{ width: 64px; display: grid; justify-items: center; gap: 3px; }}
.dock-item svg {{ width: 64px; }}
.dock-item i {{ width: 4px; height: 4px; border-radius: 50%; background: currentColor; opacity: .55; }}
.ghost {{ height: 64px; margin-bottom: 7px; border-radius: 15px; background: rgba(127,127,127,.25); transform: scale(.82); }}
.dock-wrap.light {{ color: #1D1D1F; }} .dock-wrap.dark {{ color: #F5F5F7; }}
footer {{ margin-top: 48px; color: var(--muted); font-size: 13px; }}
@media (max-width: 560px) {{
  main {{ padding-top: 32px; }}
  .card {{ padding: 14px; border-radius: 16px; }}
  .sizes {{ gap: 12px; }}
  .sizes figure:first-child svg {{ width: 96px; height: 96px; }}
  .lockups {{ grid-template-columns: 1fr; }}
  .dock-item, .ghost, .dock-item svg {{ width: 48px; }} .ghost {{ height: 48px; }}
}}
</style>
</head>
<body>
<main>
  <h1>LayeredMap logo concepts</h1>
  <p class="lede">Six directions for the macOS app icon. Each one has to say <em>map</em> and <em>several layers seen together in one place</em>. Every concept is shown large, at 128/64/32/16 px, on light and dark, in a Dock, and beside the Inter SemiBold wordmark. All six are open options.</p>

  <h3>In the Dock</h3>
  {dock("light")}
  {dock("dark")}

  <h3>Concepts</h3>
  <div class="grid">{"".join(cards)}</div>

  <footer>SVG sources and PNG renders (1024/128/32/16 px) are in <code style="margin:0">design/logo/</code> and <code style="margin:0">design/logo/png/</code>. See README.md for rendering notes.</footer>
</main>
</body>
</html>
'''
open(f"{D}/index.html", "w").write(html)
print(len(html))
