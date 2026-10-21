"""Keep the website's language links in step (web/lm).

Every page carries the same language plumbing, and it must list every language the site has:

- the `hreflang` alternates in the head (every language that has this page, plus x-default
  English), and `og:locale`;
- the language menu in the header (a `<details>` list, so it works without JavaScript);
- the list of languages in the footer;
- the app's download links (the DMG: GitHub Releases, or the GitCode mirror on Chinese pages);
- the site navigation in the header (Home, Examples, Data, Help, Download) and the footer's links
  (Help, Glossary, Pro, Changelog) with the line saying NimbleMap is an independent project.

Translated pages are written by hand; this script rewrites only those parts, from LANGS and
WORDS below, so adding a language is: add its folder of pages, add one row here, run

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

@dataclass(frozen=True)
class Words:
    """The header's and footer's link labels, and the footer's non-affiliation line."""
    home: str
    examples: str
    help: str
    glossary: str
    download: str
    pro: str
    data: str
    changelog: str
    independent: str


WORDS = {
    "": Words("Home", "Examples", "Help", "Glossary", "Download", "Pro", "Data", "Changelog",
              "NimbleMap is an independent project, not affiliated with Apple, Esri or the QGIS project."),
    "zh": Words("首页", "示例", "帮助", "术语表", "下载", "Pro 版", "数据", "更新日志",
                "凌波微图是独立项目，与苹果公司、Esri 及 QGIS 项目均无关联。"),
    "de": Words("Startseite", "Beispiele", "Hilfe", "Glossar", "Download", "Pro", "Daten", "Änderungsprotokoll",
                "NimbleMap ist ein unabhängiges Projekt und steht in keiner Verbindung zu Apple, Esri oder dem QGIS-Projekt."),
    "es": Words("Inicio", "Ejemplos", "Ayuda", "Glosario", "Descargar", "Pro", "Datos", "Registro de cambios",
                "NimbleMap es un proyecto independiente, sin relación con Apple, Esri ni el proyecto QGIS."),
    "fr": Words("Accueil", "Exemples", "Aide", "Glossaire", "Télécharger", "Pro", "Données", "Journal des modifications",
                "NimbleMap est un projet indépendant, sans lien avec Apple, Esri ni le projet QGIS."),
    "it": Words("Home", "Esempi", "Aiuto", "Glossario", "Scarica", "Pro", "Dati", "Registro delle modifiche",
                "NimbleMap è un progetto indipendente, non affiliato ad Apple, Esri o al progetto QGIS."),
    "nl": Words("Start", "Voorbeelden", "Help", "Woordenlijst", "Downloaden", "Pro", "Data", "Wijzigingslog",
                "NimbleMap is een onafhankelijk project en is niet verbonden aan Apple, Esri of het QGIS-project."),
    "pt-br": Words("Início", "Exemplos", "Ajuda", "Glossário", "Baixar", "Pro", "Dados", "Registro de alterações",
                   "O NimbleMap é um projeto independente, sem vínculo com a Apple, a Esri ou o projeto QGIS."),
    "ru": Words("Главная", "Примеры", "Справка", "Словарь", "Скачать", "Pro", "Данные", "История изменений",
                "NimbleMap — независимый проект, не связанный с Apple, Esri или проектом QGIS."),
    "ja": Words("ホーム", "サンプル", "ヘルプ", "用語集", "ダウンロード", "Pro", "データ", "変更履歴",
                "NimbleMap は独立したプロジェクトであり、Apple、Esri、QGIS プロジェクトとは関係ありません。"),
    "ko": Words("홈", "예제", "도움말", "용어집", "다운로드", "Pro", "데이터", "변경 내역",
                "NimbleMap은 독립 프로젝트이며, Apple, Esri, QGIS 프로젝트와 관련이 없습니다."),
}

CHANGELOG = "https://github.com/kangning-huang/NimbleMap/blob/main/CHANGELOG.md"

# Where the app's download buttons point, per language folder (every `href` ending in the DMG's
# file name is rewritten to this, and one ending in its .sha256 checksum file to this + ".sha256"). GitHub's /releases/latest/ alias follows each release by itself.
# Chinese pages use the GitCode mirror for mainland China, which has no "latest" alias: its URL
# names the version, so BUMP THE TAG HERE WITH EVERY RELEASE.
DMG_NAME = "NimbleMap-macos-arm64.dmg"
DMG_GITHUB = f"https://github.com/kangning-huang/NimbleMap/releases/latest/download/{DMG_NAME}"
GITCODE = "https://gitcode.com/KN-Huang/NimbleMap"
DMG_GITCODE = f"{GITCODE}/releases/download/v0.7.2/{DMG_NAME}"
DMG = {"zh": DMG_GITCODE}  # every other language: DMG_GITHUB
# The download page's second card offers the other host: GitCode everywhere, GitHub on Chinese
# pages. Its links carry data-dl="alt" (the DMG) and data-dl="alt-releases" (the releases page).
DMG_ALT = {"zh": DMG_GITHUB}  # every other language: DMG_GITCODE
RELEASES_GITHUB = "https://github.com/kangning-huang/NimbleMap/releases"
RELEASES_GITCODE = f"{GITCODE}/releases"
RELEASES_ALT = {"zh": RELEASES_GITHUB}  # every other language: RELEASES_GITCODE

# The line under the download page's main card about Pro (`<p class="dl-pro">`), one per language.
# From 0.7.1 Pro is in the one app and unlocks when you sign in.
PRO_UNLOCK = {
    "": "Pro features (the Ask box and analysis actions) unlock when you sign in inside the app, free with an account.",
    "zh": "Pro 功能（提问框和分析操作）在应用内登录后即可解锁，注册账户即可免费使用。",
    "de": "Die Pro-Funktionen (das Fragefeld und die Analyseaktionen) werden freigeschaltet, wenn Sie sich in der App anmelden – kostenlos mit einem Konto.",
    "es": "Las funciones Pro (el cuadro de preguntas y las acciones de análisis) se desbloquean al iniciar sesión dentro de la app, gratis con una cuenta.",
    "fr": "Les fonctions Pro (la zone de questions et les actions d’analyse) se débloquent quand vous vous connectez dans l’app, gratuitement avec un compte.",
    "it": "Le funzioni Pro (la casella delle domande e le azioni di analisi) si sbloccano quando accedi dall’app, gratis con un account.",
    "nl": "De Pro-functies (het vraagvak en de analyseacties) worden ontgrendeld zodra u zich in de app aanmeldt, gratis met een account.",
    "pt-br": "Os recursos Pro (a caixa de perguntas e as ações de análise) são liberados quando você entra na sua conta dentro do app, de graça com uma conta.",
    "ru": "Функции Pro (поле вопросов и действия анализа) открываются, когда вы входите в приложении, — бесплатно с учётной записью.",
    "ja": "Pro の機能（質問ボックスと分析アクション）は、アプリ内でサインインすると使えるようになります。アカウントがあれば無料です。",
    "ko": "Pro 기능(질문 상자와 분석 작업)은 앱 안에서 로그인하면 열립니다. 계정만 있으면 무료입니다.",
}

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
# The header's page links, between the <nav> and the language menu.
NAV = re.compile(r'(<nav class="nav" aria-label="[^"]*">\n)(?:[ \t]*<a [^>]*>[^<]*</a>\n)*([ \t]*<details class="lang-menu">)')
# The footer's right-hand column: the old "Help · Glossary" links, or the column written here.
DMG_LINK = re.compile(r'href="https://[^"]*/' + re.escape(DMG_NAME) + r'(\.sha256)?"( data-dl="alt")?')
RELEASES_LINK = re.compile(r'href="https://[^"]*/releases" data-dl="alt-releases"')
PRO_LINE = re.compile(r'(<p class="dl-pro">)[^<]*(<a )')
FOOT_COL = re.compile(r'<div(?: class="foot-nav")?>(?:<a href="help\.html">|<p class="foot-links">).*?<p class="foot-langs">.*?</p></div>', re.S)


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


def dmg(lang: Lang, alt: bool = False) -> str:
    """The app download URL for pages in this language (`alt`: the download page's other host)."""
    if alt:
        return DMG_ALT.get(lang.folder, DMG_GITCODE)
    return DMG.get(lang.folder, DMG_GITHUB)


def words(lang: Lang) -> Words:
    if lang.folder not in WORDS:
        raise ValueError(f"{lang.hreflang}: no row in WORDS")
    return WORDS[lang.folder]


def nav(lang: Lang, name: str) -> str:
    """The header's page links; the current section is marked (the workflows belong to Help)."""
    w = words(lang)
    here = {"index.html": "index.html", "data.html": "data.html", "help.html": "help.html",
            "download.html": "download.html"}.get(name)
    if name.startswith("workflow-"):
        here = "help.html"
    rows = [("index.html", w.home), ("index.html#examples", w.examples), ("data.html", w.data), ("help.html", w.help),
            ("download.html", w.download)]
    out = []
    for target, label in rows:
        current = ' aria-current="page"' if target == here else ""
        out.append(f'      <a href="{target}"{current}>{label}</a>\n')
    return "".join(out)


def foot_col(lang: Lang, name: str, site: Path) -> str:
    """The footer's right-hand column: links, the non-affiliation line, the other languages."""
    w = words(lang)
    if (site / path_of(lang, "pro.html")).exists():
        pro = f'<a href="pro.html">{w.pro}</a>'
    else:
        en = LANGS[0]
        pro = f'<a href="{href(lang, en, "pro.html")}" hreflang="{en.hreflang}">{w.pro}</a>'
    if (site / path_of(lang, "data.html")).exists():
        data = f'<a href="data.html">{w.data}</a>'
    else:
        en = LANGS[0]
        data = f'<a href="{href(lang, en, "data.html")}" hreflang="{en.hreflang}">{w.data}</a>'
    links = " · ".join([f'<a href="help.html">{w.help}</a>', f'<a href="glossary.html">{w.glossary}</a>', data, pro,
                        f'<a href="{CHANGELOG}">{w.changelog}</a>'])
    return (f'<div class="foot-nav"><p class="foot-links">{links}</p>'
            f'<p class="foot-note">{w.independent}</p>{foot(lang, name, site)}</div>')


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

    text, n = NAV.subn(lambda m: m.group(1) + nav(lang, name) + m.group(2), text, count=1)
    if n != 1:
        raise ValueError(f"{rel}: no site navigation in the header")
    text, n = FOOT_COL.subn(lambda _: foot_col(lang, name, site), text, count=1)
    if n != 1:
        raise ValueError(f"{rel}: no footer links column")
    text = DMG_LINK.sub(lambda m: f'href="{dmg(lang, alt=bool(m.group(2)))}{m.group(1) or ""}"{m.group(2) or ""}', text)
    text = RELEASES_LINK.sub(f'href="{RELEASES_ALT.get(lang.folder, RELEASES_GITCODE)}" data-dl="alt-releases"', text)
    text = PRO_LINE.sub(lambda m: m.group(1) + PRO_UNLOCK.get(lang.folder, PRO_UNLOCK[""])
                      + ("" if lang.folder in ("zh", "ja") else " ") + m.group(2), text)
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
