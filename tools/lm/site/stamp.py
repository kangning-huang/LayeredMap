"""Stamp the website (web/lm) for a public host.

The pages in web/lm keep every URL relative so they work from file://, offline and inside the
app (Contents/Resources/help). Search engines and link previews need absolute URLs, so publishing
goes through this script, which copies the site (minus its README) to an output folder and:

- makes the canonical, hreflang, og:url and og:image URLs absolute against ORIGIN;
- writes sitemap.xml (every page, with its language alternates);
- at a host root, also writes robots.txt (pointing at the sitemap) and the GitHub Pages `CNAME`
  file holding the host, so publishing the folder keeps the custom domain;
- publishes relative links to a folder's index.html as the folder (`zh/index.html` -> `zh/`,
  `index.html#examples` -> `./#examples`), the URL the canonical tags name; the source keeps
  index.html so the pages still open from file:// and as the app's help;
- drops the `<!-- TODO(owner): ... -->` notes, which are for us, not for visitors.

    python3 -I tools/lm/site/stamp.py /tmp/site-out                         # https://nimblemap.com/
    python3 -I tools/lm/site/stamp.py https://nimblemap.com/ /tmp/site-out  # the same, spelled out
    python3 -I tools/lm/site/stamp.py https://example.org/maps/ /tmp/site-out

Standard library only. ORIGIN (default DEFAULT_ORIGIN, the live site) is the URL the site's
index.html is served at: a host root, or a folder on it such as a GitHub Pages project site.
Crawlers only read robots.txt at a host root and a CNAME names a whole host, so both are written
only when ORIGIN is one; submit the sitemap by hand otherwise.
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
# The live site (GitHub Pages from the public NimbleMap repo's docs/, with a custom domain).
DEFAULT_ORIGIN = "https://nimblemap.com/"

# The tags whose URL must be absolute: (regex with one group for the URL).
ABSOLUTE = [
    re.compile(r'(<link rel="canonical" href=")([^"]*)(")'),
    re.compile(r'(<link rel="alternate" hreflang="[^"]*" href=")([^"]*)(")'),
    re.compile(r'(<meta property="og:url" content=")([^"]*)(")'),
    re.compile(r'(<meta property="og:image" content=")([^"]*)(")'),
]
TODO_NOTE = re.compile(r"[ \t]*<!-- TODO\(owner\):.*?-->\n?", re.S)
INDEX_LINK = re.compile(r'(<a [^>]*?href=")((?:\.\./)*(?:[a-z-]+/)?)index\.html((?:#[^"]*)?")')
ALTERNATE = re.compile(r'<link rel="alternate" hreflang="([^"]*)" href="([^"]*)">')


def page_url(origin: str, rel: str) -> str:
    """The public URL of a page: `index.html` is served as its folder."""
    url = f"{origin}/{rel}"
    return url[: -len("index.html")] if url.endswith("/index.html") else url


def folder_links(text: str) -> str:
    """Relative `<a href>`s to an index.html become the folder (`index.html` alone: `./`)."""
    return INDEX_LINK.sub(lambda m: m.group(1) + (m.group(2) or "./") + m.group(3), text)


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
    if parts.scheme != "https" or not parts.netloc or parts.query or parts.fragment or parts.path.endswith(".html"):
        raise ValueError(f"ORIGIN must be an https site URL such as https://example.org, not {origin!r}")
    origin = origin.rstrip("/")
    if out.exists():
        raise ValueError(f"{out} already exists; give a new folder")
    shutil.copytree(site, out, ignore=shutil.ignore_patterns("README.md", "._*", ".DS_Store"))

    entries = []
    for page in sorted(out.rglob("*.html")):
        rel = page.relative_to(out).as_posix()
        base = f"{origin}/{rel}"
        text = TODO_NOTE.sub("", folder_links(absolutize(page.read_text(encoding="utf-8"), base, origin)))
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
    if parts.path in ("", "/"):
        (out / "robots.txt").write_text(f"User-agent: *\nAllow: /\n\nSitemap: {origin}/sitemap.xml\n", encoding="utf-8")
        (out / "CNAME").write_text(parts.netloc + "\n", encoding="utf-8")
    return [loc for loc, _ in entries]


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__.strip().splitlines()[0] + "\nusage: stamp.py [ORIGIN] OUT_DIR   (ORIGIN defaults to " + DEFAULT_ORIGIN + ")",
              file=sys.stderr)
        return 2
    origin, out = (argv[1], argv[2]) if len(argv) == 3 else (DEFAULT_ORIGIN, argv[1])
    if "://" in out:
        print(f"stamp.py: OUT_DIR must be a folder, not {out!r}", file=sys.stderr)
        return 2
    try:
        pages = stamp(origin, Path(out))
    except ValueError as e:
        print(f"stamp.py: {e}", file=sys.stderr)
        return 2
    extra = ("sitemap.xml, robots.txt and CNAME" if (Path(out) / "robots.txt").exists()
             else "sitemap.xml (no robots.txt or CNAME below a host root)")
    print(f"{len(pages)} pages stamped into {out} for {origin}, with {extra}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
