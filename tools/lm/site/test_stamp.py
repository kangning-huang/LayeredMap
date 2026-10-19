"""Tests for stamp.py, and checks that every page carries the head tags it needs.

    python3 -I -m pytest -q tools/lm/site
"""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import langs  # noqa: E402
import stamp  # noqa: E402

import pytest  # noqa: E402

PAGES = sorted(p.relative_to(stamp.SITE).as_posix() for p in stamp.SITE.rglob("*.html") if not p.name.startswith("._"))


def test_rejects_origins_that_are_not_https_sites(tmp_path):
    for bad in ["http://example.org", "https://example.org/index.html", "example.org", "https://", "https://example.org/?q=1"]:
        with pytest.raises(ValueError):
            stamp.stamp(bad, tmp_path / "out")


def test_stamps_a_project_site_below_the_host_root(tmp_path):
    out = tmp_path / "out"
    stamp.stamp("https://someone.github.io/NimbleMap/", out)
    home = (out / "index.html").read_text(encoding="utf-8")
    assert '<link rel="canonical" href="https://someone.github.io/NimbleMap/">' in home
    assert '<link rel="alternate" hreflang="zh-Hans" href="https://someone.github.io/NimbleMap/zh/">' in home
    assert "<loc>https://someone.github.io/NimbleMap/download.html</loc>" in (out / "sitemap.xml").read_text()
    assert not (out / "robots.txt").exists()


def test_drops_owner_notes(tmp_path):
    out = tmp_path / "out"
    stamp.stamp("https://maps.example", out)
    for page in out.rglob("*.html"):
        assert "TODO(owner)" not in page.read_text(encoding="utf-8"), page


def test_stamps_absolute_urls_sitemap_and_robots(tmp_path):
    out = tmp_path / "out"
    pages = stamp.stamp("https://maps.example/", out)
    assert len(pages) == len(PAGES)
    assert not (out / "README.md").exists()
    home = (out / "index.html").read_text(encoding="utf-8")
    assert '<link rel="canonical" href="https://maps.example/">' in home
    assert '<link rel="alternate" hreflang="zh-Hans" href="https://maps.example/zh/">' in home
    assert '<meta property="og:image" content="https://maps.example/img/typhoons-card.jpg">' in home
    zh = (out / "zh" / "download.html").read_text(encoding="utf-8")
    assert '<link rel="alternate" hreflang="en" href="https://maps.example/download.html">' in zh
    assert '<meta property="og:image" content="https://maps.example/img/walls-compare.jpg">' in zh
    sitemap = (out / "sitemap.xml").read_text(encoding="utf-8")
    assert "<loc>https://maps.example/zh/workflow-heatwave.html</loc>" in sitemap
    assert sitemap.count("<url>") == len(PAGES)
    assert "Sitemap: https://maps.example/sitemap.xml" in (out / "robots.txt").read_text()
    # Nothing relative is left in the stamped tags.
    for page in out.rglob("*.html"):
        for pat in stamp.ABSOLUTE:
            for m in pat.finditer(page.read_text(encoding="utf-8")):
                assert m.group(2).startswith("https://maps.example/"), (page, m.group(0))


@pytest.mark.parametrize("rel", PAGES)
def test_every_page_has_search_and_preview_tags(rel):
    text = (stamp.SITE / rel).read_text(encoding="utf-8")
    name = Path(rel).name
    assert f'<link rel="canonical" href="{name}">' in text
    # Every language that has this page is an alternate, plus English as the default.
    have = {lang.hreflang for lang in langs.present(stamp.SITE) if (stamp.SITE / langs.path_of(lang, name)).exists()}
    found = set(re.findall(r'<link rel="alternate" hreflang="([^"]*)"', text))
    assert found == have | {"x-default"}, found
    assert '<link rel="alternate" hreflang="x-default" href="' + langs.href(langs.lang_of(rel), langs.LANGS[0], name) + '">' in text
    for tag in ["og:title", "og:description", "og:url", "og:image"]:
        assert f'property="{tag}"' in text, tag
    title = re.search(r"<title>(.*?)</title>", text).group(1)
    assert len(title) <= 75, title
    # Placeholders stay in HTML comments, never on the page.
    assert 'class="todo"' not in text
    # Every linked page and image in the site exists.
    for href in re.findall(r'(?:href|src|content)="([^"#:]+\.(?:html|jpg|svg|css|js))(?:#[^"]*)?"', text):
        assert (stamp.SITE / rel).parent.joinpath(href).resolve().exists(), href


LANG_PAGES = [lang for lang in langs.present(stamp.SITE)]


def test_every_language_has_its_core_pages():
    """A language is listed once it has pages; it needs at least the landing and download pages."""
    assert {"en", "zh-Hans"} <= {lang.hreflang for lang in LANG_PAGES}
    for lang in LANG_PAGES:
        for name in ["index.html", "download.html"]:
            assert (stamp.SITE / langs.path_of(lang, name)).exists(), (lang.hreflang, name)


def test_language_links_are_up_to_date():
    """Every page's alternates, language menu and footer list match LANGS (run langs.py to fix)."""
    assert langs.run(check=True) == []


@pytest.mark.parametrize("rel", PAGES)
def test_every_page_speaks_its_folders_language(rel):
    text = (stamp.SITE / rel).read_text(encoding="utf-8")
    lang = langs.lang_of(rel)
    assert f'<html lang="{lang.hreflang}">' in text
    assert f'<meta property="og:locale" content="{lang.og}">' in text
    # The menu and the footer list every language, each linking to a page that exists.
    menu = re.search(r'<details class="lang-menu">(.*?)</details>', text, re.S).group(1)
    foot = re.search(r'<p class="foot-langs">(.*?)</p>', text, re.S).group(1)
    for other in LANG_PAGES:
        assert f'hreflang="{other.hreflang}"' in menu, other.hreflang
        assert (other == lang) or f'hreflang="{other.hreflang}"' in foot, other.hreflang
    assert menu.count('aria-current="true"') == 1
    # Structured data names its language.
    for block in re.findall(r'<script type="application/ld\+json">(.*?)</script>', text, re.S):
        assert f'"inLanguage": "{lang.hreflang}"' in block, rel
    # Visible text in a language other than English doesn't keep the English navigation.
    if lang.hreflang != "en":
        nav = re.search(r'<nav class="nav" aria-label="([^"]*)"', text).group(1)
        assert nav != "Site", rel
