"""Keep the website's language links in step (web/lm).

Every page carries the same language plumbing, and it must list every language the site has:

- the `hreflang` alternates in the head (every language that has this page, plus x-default
  English), and `og:locale`;
- the language menu in the header (a `<details>` list, so it works without JavaScript);
- the list of languages in the footer.

Translated pages are written by hand; this script rewrites only those three parts, from
LANGS below, so adding a language is: add its folder of pages, add one row here, run

    python3 -I tools/lm/site/langs.py          # rewrite the pages in place
    python3 -I tools/lm/site/langs.py --check  # exit 1 if a page is out of date (the tests run this)

Standard library only. English lives at the site root; every other language in a folder.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path

SITE = Path(__file__).resolve().parents[3] / "web" / "lm"


@dataclass(frozen=True)
class Lang:
    folder: str  # "" for English at the root
    hreflang: str  # BCP 47, also the page's <html lang>
    og: str  # og:locale
    name: str  # the language's name in itself, as the menu lists it
    short: str  # the menu button's label
    word: str  # "Language", in this language (the menu button's accessible name)


# English first (the default), then Chinese, then the rest in the order the menu lists them.
LANGS = [
    Lang("", "en", "en_US", "English", "EN", "Language"),
    Lang("zh", "zh-Hans", "zh_CN", "简体中文", "中文", "语言"),
    Lang("de", "de", "de_DE", "Deutsch", "DE", "Sprache"),
    Lang("es", "es", "es_ES", "Español", "ES", "Idioma"),
    Lang("fr", "fr", "fr_FR", "Français", "FR", "Langue"),
    Lang("it", "it", "it_IT", "Italiano", "IT", "Lingua"),
    Lang("nl", "nl", "nl_NL", "Nederlands", "NL", "Taal"),
    Lang("pt-br", "pt-BR", "pt_BR", "Português (Brasil)", "PT", "Idioma"),
    Lang("ru", "ru", "ru_RU", "Русский", "RU", "Язык"),
    Lang("ja", "ja", "ja_JP", "日本語", "日本語", "言語"),
    Lang("ko", "ko", "ko_KR", "한국어", "한국어", "언어"),
]

GLOBE = (
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">'
    '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.6 2.6 3.9 5.6 3.9 9s-1.3 6.4-3.9 9'
    'c-2.6-2.6-3.9-5.6-3.9-9S9.4 5.6 12 3z"/></svg>'
)

ALTERNATES = re.compile(r'(?:<link rel="alternate" hreflang="[^"]*" href="[^"]*">\n)+')
CANONICAL = re.compile(r'<link rel="canonical" href="[^"]*">\n')
OG_LOCALE = re.compile(r'<meta property="og:locale" content="[^"]*">\n')
OG_SITE = re.compile(r'<meta property="og:site_name" content="[^"]*">\n')
# The header's language control: the old single link, or the menu this script writes.
MENU = re.compile(r'<a class="lang" [^>]*>[^<]*</a>|<details class="lang-menu">.*?</details>', re.S)
# The footer's language links: the old single link after the glossary, or the list written here.
FOOT_OLD = re.compile(r'( · <a href="[^"]*" lang="[^"]*">[^<]*</a>)(</div>\n  </div>\n</footer>)')
FOOT = re.compile(r'<p class="foot-langs">.*?</p>', re.S)
FOOT_END = re.compile(r"(</div>\n  </div>\n</footer>)")


def lang_of(rel: str) -> Lang:
    """The language of a page, from its folder."""
    folder = rel.split("/")[0] if "/" in rel else ""
    for lang in LANGS:
        if lang.folder == folder:
            return lang
    raise ValueError(f"{rel}: no row in LANGS for folder {folder!r}")


def path_of(lang: Lang, name: str) -> str:
    """A page's path from the site root."""
    return f"{lang.folder}/{name}" if lang.folder else name


def href(src: Lang, dst: Lang, name: str) -> str:
    """The relative link from a page in `src` to page `name` in `dst`."""
    if src == dst:
        return name
    up = "../" if src.folder else ""
    return up + path_of(dst, name)


def present(site: Path) -> list[Lang]:
    """The languages the site has pages in (a row in LANGS without pages yet is skipped)."""
    return [lang for lang in LANGS if any((site / path_of(lang, n)).exists() for n in HOMES)]


# Where a language's links go when it lacks the page itself: the first of these it has.
HOMES = ["index.html", "download.html", "help.html"]


def page_or_home(dst: Lang, name: str, site: Path) -> str:
    """`name` in `dst` if that language has it, else its home page."""
    for n in [name, *HOMES]:
        if (site / path_of(dst, n)).exists():
            return n
    raise ValueError(f"{dst.hreflang}: no pages")


def alternates(lang: Lang, name: str, site: Path) -> str:
    rows = [(other.hreflang, href(lang, other, name)) for other in present(site) if (site / path_of(other, name)).exists()]
    rows.append(("x-default", href(lang, LANGS[0], name)))
    return "".join(f'<link rel="alternate" hreflang="{h}" href="{u}">\n' for h, u in rows)


def menu(lang: Lang, name: str, site: Path) -> str:
    colon = "：" if lang.hreflang in ("zh-Hans", "ja") else ": "
    items = []
    for other in present(site):
        target = href(lang, other, page_or_home(other, name, site))
        current = ' aria-current="true"' if other == lang else ""
        items.append(f'<li><a href="{target}" hreflang="{other.hreflang}" lang="{other.hreflang}"{current}>{other.name}</a></li>')
    return (
        '<details class="lang-menu">\n'
        f'        <summary aria-label="{lang.word}{colon}{lang.name}" title="{lang.word}">{GLOBE}<span>{lang.short}</span></summary>\n'
        '        <ul>\n          ' + "\n          ".join(items) + "\n        </ul>\n      </details>"
    )


def foot(lang: Lang, name: str, site: Path) -> str:
    links = []
    for other in present(site):
        if other == lang:
            continue
        target = href(lang, other, page_or_home(other, name, site))
        links.append(f'<a href="{target}" hreflang="{other.hreflang}" lang="{other.hreflang}">{other.name}</a>')
    return f'<p class="foot-langs">{" · ".join(links)}</p>'


def rewrite(text: str, rel: str, site: Path = SITE) -> str:
    """The page with its language links rebuilt; everything else is left as it is."""
    lang = lang_of(rel)
    name = Path(rel).name
    if f'<html lang="{lang.hreflang}">' not in text:
        raise ValueError(f'{rel}: expected <html lang="{lang.hreflang}">')

    text = ALTERNATES.sub("", text, count=1)
    m = CANONICAL.search(text)
    if not m:
        raise ValueError(f"{rel}: no canonical link")
    text = text[: m.end()] + alternates(lang, name, site) + text[m.end() :]

    locale = f'<meta property="og:locale" content="{lang.og}">\n'
    if OG_LOCALE.search(text):
        text = OG_LOCALE.sub(locale, text, count=1)
    else:
        m = OG_SITE.search(text)
        if not m:
            raise ValueError(f"{rel}: no og:site_name")
        text = text[: m.end()] + locale + text[m.end() :]

    text, n = MENU.subn(lambda _: menu(lang, name, site), text, count=1)
    if n != 1:
        raise ValueError(f"{rel}: no language link in the header")

    text = FOOT_OLD.sub(r"\2", text, count=1)
    if FOOT.search(text):
        text = FOOT.sub(lambda _: foot(lang, name, site), text, count=1)
    else:
        text, n = FOOT_END.subn(lambda m: foot(lang, name, site) + m.group(1), text, count=1)
        if n != 1:
            raise ValueError(f"{rel}: footer not found")
    return text


def pages(site: Path = SITE) -> list[str]:
    return sorted(p.relative_to(site).as_posix() for p in site.rglob("*.html") if not p.name.startswith("._"))


def run(check: bool, site: Path = SITE) -> list[str]:
    """Rewrites every page (or, with `check`, only reports); returns the pages that changed."""
    changed = []
    for rel in pages(site):
        path = site / rel
        old = path.read_text(encoding="utf-8")
        new = rewrite(old, rel, site)
        if new != old:
            changed.append(rel)
            if not check:
                path.write_text(new, encoding="utf-8")
    return changed


def main(argv: list[str]) -> int:
    check = "--check" in argv[1:]
    try:
        changed = run(check)
    except ValueError as e:
        print(f"langs.py: {e}", file=sys.stderr)
        return 2
    if check and changed:
        print("out of date (run tools/lm/site/langs.py):\n  " + "\n  ".join(changed), file=sys.stderr)
        return 1
    print(f"{len(changed)} pages {'need updating' if check else 'updated'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
