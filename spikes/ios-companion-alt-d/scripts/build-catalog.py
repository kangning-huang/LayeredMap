#!/usr/bin/env python3
"""Turn web/lm/data.html into a catalogue the iPhone app can search offline.

The site already carries a hand-written catalogue — 9 themed sections, real titles people
recognise ("Fires burning right now", "Roads of the Roman Empire"), the source, the formats, the
licence and, for most entries, a direct file URL. The app could not search any of it: it had four
bundled sets and a web view. Lifting it into a bundled JSON means most searches are answered with
no network call and no model at all.

Only entries with a direct file link are kept: an entry whose only action is "Source page ↗" sends
someone to a portal to hunt, which is the dead end this whole spike exists to remove.
"""
import html
import json
import pathlib
import re
import sys

SRC = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "web/lm/data.html")
OUT = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else
                   "spikes/ios-companion-alt-d/ios/NimbleMapAltD/catalog.json")

# Extensions the app can open (Importer.extensions + RasterDecode.extensions).
OPENABLE = {"geojson", "json", "csv", "tsv", "txt", "kml", "kmz", "gpx", "zip", "gpkg", "shp",
            "tif", "tiff", "asc", "nc", "nc4", "cdf"}

text = SRC.read_text()


def clean(s: str) -> str:
    s = re.sub(r"<[^>]+>", "", s)
    return re.sub(r"\s+", " ", html.unescape(s)).strip()


def extension_of(url: str) -> str:
    path = url.split("?")[0].split("#")[0]
    return path.rsplit(".", 1)[-1].lower() if "." in path.rsplit("/", 1)[-1] else ""


sections = {}
for m in re.finditer(r'<section id="([^"]+)" class="ds-section">(.*?)</section>', text, re.S):
    sid, body = m.group(1), m.group(2)
    h2 = re.search(r"<h2>(.*?)</h2>", body, re.S)
    sections[sid] = (clean(h2.group(1)) if h2 else sid, body)

items, skipped = [], []
for sid, (section_name, body) in sections.items():
    for art in re.findall(r'<article class="ds">(.*?)</article>', body, re.S):
        title_m = re.search(r"<h3>(.*?)</h3>", art, re.S)
        if not title_m:
            continue
        title = clean(title_m.group(1))
        src_m = re.search(r'<p class="ds-src">(.*?)</p>', art, re.S)
        why_m = re.search(r'<p class="ds-why">(.*?)</p>', art, re.S)
        chips = [clean(c) for c in re.findall(r"<li>(.*?)</li>", art, re.S)]
        get = re.search(r'<a class="ds-get" href="([^"]+)"[^>]*>(.*?)</a>', art, re.S)
        if not get:
            skipped.append((title, "no direct download"))
            continue
        url, label = html.unescape(get.group(1)), clean(get.group(2))
        ext = extension_of(url)
        if ext and ext not in OPENABLE:
            skipped.append((title, f"format .{ext} not openable"))
            continue
        size = ""
        if "·" in label:
            size = label.rsplit("·", 1)[1].strip()
        items.append({
            "id": re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-"),
            "title": title,
            "blurb": clean(why_m.group(1)) if why_m else "",
            "source": clean(src_m.group(1)) if src_m else "",
            "format": chips[0] if chips else (ext.upper() if ext else ""),
            "size": size,
            "licence": chips[2] if len(chips) > 2 else (chips[-1] if chips else ""),
            "section": section_name,
            "url": url,
        })

OUT.write_text(json.dumps(items, indent=1, ensure_ascii=False) + "\n")
print(f"{len(items)} datasets -> {OUT}")
for t, why in skipped:
    print(f"  skipped: {t} ({why})")
