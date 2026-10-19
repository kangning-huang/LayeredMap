"""Stamp the website (web/lm) for a public host.

The pages in web/lm keep every URL relative so they work from file://, offline and inside the
app (Contents/Resources/help). Search engines and link previews need absolute URLs, so publishing
goes through this script, which copies the site (minus its README) to an output folder and:

- makes the canonical, hreflang, og:url and og:image URLs absolute against ORIGIN;
- writes sitemap.xml (every page, with its language alternates) and robots.txt.

    python3 -I tools/lm/site/stamp.py https://example.org /tmp/site-out

Standard library only. ORIGIN is the site's root URL, without a trailing path.
"""

from __future__ import annotations

import re
import shutil
import sys
from datetime import date
from pathlib import Path
from urllib.parse import urljoin, urlsplit
from xml.sax.saxutils import escape

SITE = Path(__file__).resolve().parents[3] / "web" / "lm"

# The tags whose URL must be absolute: (regex with one group for the URL).
ABSOLUTE = [
    re.compile(r'(<link rel="canonical" href=")([^"]*)(")'),
    re.compile(r'(<link rel="alternate" hreflang="[^"]*" href=")([^"]*)(")'),
    re.compile(r'(<meta property="og:url" content=")([^"]*)(")'),
    re.compile(r'(<meta property="og:image" content=")([^"]*)(")'),
]
ALTERNATE = re.compile(r'<link rel="alternate" hreflang="([^"]*)" href="([^"]*)">')


def page_url(origin: str, rel: str) -> str:
    """The public URL of a page: `index.html` is served as its folder."""
    url = f"{origin}/{rel}"
    return url[: -len("index.html")] if url.endswith("/index.html") else url


def absolutize(text: str, base: str, origin: str) -> str:
    def fix(m: re.Match) -> str:
        url = urljoin(base, m.group(2))
        if url.startswith(origin + "/") and url.endswith("/index.html"):
            url = url[: -len("index.html")]
        return m.group(1) + url + m.group(3)

    for pat in ABSOLUTE:
        text = pat.sub(fix, text)
    return text


def stamp(origin: str, out: Path, site: Path = SITE) -> list[str]:
    parts = urlsplit(origin)
    if parts.scheme != "https" or not parts.netloc or parts.path not in ("", "/") or parts.query:
        raise ValueError(f"ORIGIN must be an https root URL such as https://example.org, not {origin!r}")
    origin = origin.rstrip("/")
    if out.exists():
        raise ValueError(f"{out} already exists; give a new folder")
    shutil.copytree(site, out, ignore=shutil.ignore_patterns("README.md", "._*", ".DS_Store"))

    entries = []
    for page in sorted(out.rglob("*.html")):
        rel = page.relative_to(out).as_posix()
        base = f"{origin}/{rel}"
        text = absolutize(page.read_text(encoding="utf-8"), base, origin)
        page.write_text(text, encoding="utf-8")
        alternates = ALTERNATE.findall(text)
        entries.append((page_url(origin, rel), alternates))

    today = date.today().isoformat()
    lines = ['<?xml version="1.0" encoding="UTF-8"?>',
             '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">']
    for loc, alternates in entries:
        lines.append(f"  <url><loc>{escape(loc)}</loc><lastmod>{today}</lastmod>")
        for lang, href in alternates:
            lines.append(f'    <xhtml:link rel="alternate" hreflang="{escape(lang)}" href="{escape(href)}"/>')
        lines.append("  </url>")
    lines.append("</urlset>")
    (out / "sitemap.xml").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (out / "robots.txt").write_text(f"User-agent: *\nAllow: /\n\nSitemap: {origin}/sitemap.xml\n", encoding="utf-8")
    return [loc for loc, _ in entries]


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__.strip().splitlines()[0] + "\nusage: stamp.py ORIGIN OUT_DIR", file=sys.stderr)
        return 2
    try:
        pages = stamp(argv[1], Path(argv[2]))
    except ValueError as e:
        print(f"stamp.py: {e}", file=sys.stderr)
        return 2
    print(f"{len(pages)} pages stamped into {argv[2]}, with sitemap.xml and robots.txt")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
