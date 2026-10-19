# Localization (WP5.5 and the website)

NimbleMap's interface and website in eleven languages: English, Simplified Chinese, and nine
added on 2026-10-19. **Every translation other than English and the existing Chinese pages is a
machine translation that needs native review before release** (checklists at the end).

## 1. Which languages, and why

The owner asked for French, Spanish, German, Japanese and Korean, plus the languages the most
geospatial-data users speak. Constraints: prefer languages upstream RustGIS already ships
catalogs for (az, de, es, fr, id, it, ja, ka, ko, nl, pt, ru, tr, vi, zh), and leave out scripts
egui can't shape or lay out (Arabic, Persian, Hebrew, Hindi, Thai).

**Chosen (9 + English + Chinese):** German `de`, Spanish `es`, French `fr`, Italian `it`, Dutch
`nl`, Brazilian Portuguese `pt-BR`, Russian `ru`, Japanese `ja`, Korean `ko`.

| Language | Evidence |
|---|---|
| German | Most active OpenStreetMap contributors of any country by far (1,196 on 2026-10-18, against 936 for the US and 499 for France) [OSMstats]; QGIS user group (QGIS Anwendergruppe Deutschland) [QGIS groups]. QGIS docs only 55% translated [QGIS docs], a sign that many German users work in English, but the market is the largest in Europe. |
| French | 2nd in OSM contributors in Europe (499) [OSMstats]; QGIS user group France [QGIS groups]; QGIS docs 78% translated [QGIS docs]; also reaches Francophone Africa, Belgium, Switzerland and Québec. |
| Spanish | OSM 210 (Spain alone) [OSMstats]; QGIS docs 92% translated [QGIS docs]; QGIS user groups in Spain, Argentina, Colombia, Ecuador, Mexico (two) and Peru [QGIS groups], the widest spread of any language after English. |
| Italian | OSM 280 [OSMstats]; QGIS docs 94% translated [QGIS docs]; Italian QGIS user group [QGIS groups]. |
| Dutch | OSM 142 for a small population [OSMstats]; QGIS docs 100% translated, the only complete one [QGIS docs]; Dutch QGIS user group [QGIS groups]. The weakest case of the nine (Dutch readers read English well): the first to drop if the list must shrink. |
| Brazilian Portuguese | QGIS Brasil, one of the oldest official user groups, reports 40,000+ members and gave QGIS 4.2 its name, "Belém do Pará" [QGIS Brasil talk] [Diário do Pará]; QGIS Portugal too [QGIS groups]; QGIS docs 57% [QGIS docs]; OSM 92 [OSMstats]. |
| Russian | OSM 191 contributors with the most edits per contributor [OSMstats]; QGIS docs 25% [QGIS docs]; covers Russian-reading users across Eastern Europe and Central Asia. Owner's call: Mac share is low there and Pro payments may not be possible; the free app and the site are unaffected. |
| Japanese | Requested. OSM 181 [OSMstats]; QGIS docs 93% [QGIS docs]; QGIS User Group Japan (OSGeo.JP) [QGIS groups]; a large Japanese QGIS readership (MIERUNE's QGIS LAB, ~20k readers a month) [FOSS4G 2026]. |
| Korean | Requested. QGIS docs 87% translated [QGIS docs], although OSM is small (36) [OSMstats] and there is no listed QGIS user group. |

**Considered and left out for now:** Indonesian (large humanitarian-mapping community, but OSM 39,
QGIS docs 3.8%, a mostly Android market; the first to add next), Turkish (OSM 60, QGIS docs
3.4%), Polish (OSM 235, the strongest candidate not in upstream's catalog set; would need a new
upstream catalog), Vietnamese (OSM 25, 1.7%), Azerbaijani and Georgian (0.1–0.2%).

Sources:
[OSMstats]: https://osmstats.neis-one.org/?item=countries (contributors per country, 2026-10-18; counted from changeset centres, ±2–10%) ·
[QGIS docs]: https://docs.qgis.org/latest/en/docs/about/contributors.html ("Statistics of translation", QGIS 3.44 LTR documentation) ·
[QGIS groups]: https://qgis.org/community/groups/list/ (39 active groups in 37 countries) ·
[QGIS Brasil talk]: https://talks.osgeo.org/foss4g-2026/talk/CQHMXX/ and https://talks.osgeo.org/qgis-uc2026/talk/LQWMTY/ ·
[Diário do Pará]: https://diariodopara.com.br/cotidiano/qgis-belem-do-para-da-nome-a-nova-versao-do-software-livre-pela-primeira-vez/ ·
[FOSS4G 2026]: https://talks.osgeo.org/foss4g-2026/talk/UWSLUK/

Not found: per-country QGIS download numbers (QGIS has collected plugin-download countries only
since May 2024 and doesn't publish them, https://www.qgis.org/resources/support/privacy) and Mac
share per country.

## 2. The brand in Japanese and Korean

**Recommendation: "NimbleMap" in both, no kanji, hanja or hangul form.** Implemented on the
Japanese and Korean pages and in the app; `lm_core::brand` gets no Japanese or Korean constant
(`NAME` stays "NimbleMap").

**Chinese (owner's decision, 2026-10-19):** the Chinese interface shows 凌波微图 (Pro: 凌波微图
Pro) wherever the app writes its own name: the toolbar, menus (About, Quit, Help), messages, the
export caption ("Made with …"), the update notice, the Save dialog's file type.
`lm_core::brand::shown(pro)` picks it by the interface language; files, logs, diagnostics reports
and the `software` field keep "NimbleMap". On macOS, Finder, the Dock and the menu bar's
application menu take the name from the bundle: `bundle-macos.sh` writes
`zh-Hans.lproj/InfoPlist.strings` (CFBundleName, CFBundleDisplayName = 凌波微图) and sets
`LSHasLocalizedDisplayName`, so they show 凌波微图 when **the Mac's** language is Chinese; they
can't follow the app's own View › Language. The window title is the map's name, and the
`.app` folder stays "NimbleMap.app". Windows: the executable's file description and the Start
menu entry stay "NimbleMap" (no per-language resources yet).

Japanese, 凌波微図:
- Jin Yong is published in Japan (Tokuma Shoten's 金庸武俠小説集, 1996–2004, with 天龍八部 in
  eight volumes; reissued in Tokuma Bunko) but read by a niche of wuxia and Chinese-history fans
  (https://ja.wikipedia.org/wiki/金庸). 凌波微歩 is not a term Japanese readers know; it isn't
  mentioned in the Japanese article on the author.
- 凌波 (りょうは) is a rare literary word; the one dictionary sense found is 凌波性, a ship's
  seakeeping. 微図 is not a Japanese word. 凌波微図 would read as an opaque coined compound, and
  the pun (which needs 凌波微步) would be lost.
- Western software keeps its Latin name in Japan (QGIS, ArcGIS, Figma, Notion, Procreate), with a
  descriptive Japanese subtitle in stores; only a few household names get katakana
  (Google マップ) (https://www.apptweak.com/ja/aso-blog/how-to-localize-your-app-in-japanese).
- A katakana reading (ニンブルマップ) adds nothing for search: "nimble" isn't a loanword people
  know. If the owner wants one for an App Store subtitle, that is the place for it, not the name.

Korean, 凌波微圖 / 능파미도:
- Hanja literacy among younger Koreans is low: newspapers moved to hangul-only in the 1990s and
  use hanja only as an occasional gloss; schools teach a limited set
  (https://www.ritsumei.ac.jp/ir/isaru/assets/file/journal/30-2_06LEE.pdf,
  https://sgsg.hankyung.com/article/2026043078151). 凌波微圖 would be unreadable to most readers.
- Jin Yong (김용) is far better known in Korea than in Japan (the 영웅문 trilogy;
  https://dhnews.co.kr/news/view/1065577449201563; 천룡팔부 is sold in full translation), but
  능파미보 is known to martial-arts readers, mostly older ones, not to the general public.
- The hangul 능파미도 carries no meaning (미도 suggests nothing like "map"), so it would be a
  meaningless string, not a pun.
- Western apps keep Latin names in Korea too (Notion, Figma, QGIS), except a few household names
  (구글 지도).

Trademark and search: no product named 凌波 turned up in a web search, but J-PlatPat and KIPRIS
were not reachable from here; a manual check is part of the pending trademark search
(docs/lm/naming.md). Keeping one Latin name in ten languages also keeps one trademark to clear.

## 3. Website (web/lm)

| Language | Folder | Pages |
|---|---|---|
| English | `/` | 7 |
| 简体中文 | `zh/` | 7 (existing) |
| Deutsch, Español, Français, Italiano, Nederlands, Português (Brasil), Русский, 日本語, 한국어 | `de/ es/ fr/ it/ nl/ pt-br/ ru/ ja/ ko/` | 7 each: home, download, help, glossary, three workflows |

- Every page lists every language as an `hreflang` alternate (plus `x-default` English), sets
  `og:locale`, and its JSON-LD names `inLanguage`. `tools/lm/site/langs.py` writes the alternates,
  the header's language menu and the footer's language list from one table; `test_stamp.py` runs
  it in `--check` mode and checks every page (titles ≤ 75 characters, every alternate, menu and
  footer complete, links resolve).
- The language menu is a `<details>` list (no JavaScript needed; `site.js` only adds closing on
  Esc or a click elsewhere), two columns, hung from the header's right edge, inside the 16px
  gutters on a phone; on a phone only the globe shows. In long languages the four nav links and
  the two buttons wrap to a second row at 360 px; there is no horizontal scroll.
- The glossary page reads `assets/glossary-<folder>.js` (term, other names, meaning, where in the
  app); search matches the page's language, English and Chinese.
- Japanese and Korean have their own font stacks (Hiragino/Yu Gothic/Meiryo; Apple SD Gothic Neo/
  Malgun Gothic) so kanji don't take Chinese shapes; Korean breaks between words.
- **App labels on the pages** stay in English, as the screenshots show them, with the language's
  term as a gloss: `<span class="ui">Layers</span> (Couches)`, the way the Chinese pages do. The
  glosses come from the per-language term lists (below), which the app's catalogs also use. When
  a language's interface is reviewed, swap the labels to it and retake the screenshots.
- **Ask examples** stay in English with a translation in parentheses: the Ask box takes English
  or Chinese only.
- Prices in JSON-LD use the local currency (EUR, BRL, RUB, JPY, KRW), still "0".
- Small additions a reviewer should confirm or remove: pt-BR adds "no hemisfério norte" to the
  summer-2023 heatwave (June–August is winter in Brazil); ja glosses the Qing administrative ranks
  on the walled-cities page; several titles reorder or shorten words to fit 75 characters (fr
  drops GeoTIFF from the home title).

## 4. The app interface

**How it works.** `lm_core::i18n` (`crates/lm-core/src/i18n.rs`) is a thin NimbleMap layer with
upstream's design (`crates/ui-egui/src/i18n`): English strings in the code are the keys; one
`<code>.tsv` catalog per language; `tl!("…")`, `i18n::fmt("… {n} …", …)`; a missing entry shows
in English. Upstream's module itself can't be reused: it lives in `rustgis-ui-egui`, the ribbon
front end the module table keeps `lm-*` off, and its 1,874-string catalog is the ribbon's own
text (its terms were a reference for ours). Same format and rules, so a translator who knows one
knows both. `tn!("…")` marks an English label that is also an id (matched in code) and is
translated where it is drawn.

- **Language**: follows the system's preferred languages (the first with a catalog; Traditional
  Chinese locales fall back to English), with an override in **View › Language** (stored as
  `lm.language`; "System Language" goes back to auto). `LM_LANGUAGE=ja` for one run; the `--tour`
  step `language ja`. Changing it relabels the menu bar at once. Help › NimbleMap Help opens the
  help page in the interface's language.
- **Numbers** the interface formats itself follow the language (`i18n::count`, `i18n::number`:
  1.248,5 in German, 1 248,5 in French with a narrow no-break space, 14 537 but 4999 in Spanish with a no-break space and the decimal point, as on the Spanish site);
  English is unchanged. Dataset values, table cells, exports and saved files are never
  translated or reformatted.
- **Never translated**: field and layer names, file contents, dataset values, command and menu
  ids, saved documents and recipes, the CLI, logs, the diagnostics report, `--tour` words, and the
  brand (catalogs say `{app}`, filled from `lm_core::brand`).
- **Catalogs**: 709 strings, all ten languages complete (`crates/lm-core/src/i18n/{zh,de,es,fr,
  it,nl,pt,ru,ja,ko}.tsv`). `pt` is Brazilian Portuguese.
- **Tests**: `lm-core` checks every catalog parses, keeps each key's `{placeholders}`, `[tokens]`
  and line breaks, and never types the brand; numbers per language. `crates/lm-ui/tests/
  catalogs.rs` reads the sources: every marked key (and every `rules.toml` action label) is in
  every catalog, no catalog keeps a key the code dropped, and English handed straight to a widget
  (`.label("…")`, `Button::new("…")`, `on_hover_text("…")`, menu items…) without translation fails
  the build unless the line says `// i18n: <reason>`. All English pixel snapshots are unchanged.
- **Review aid**: `LM_GALLERY=dir cargo test -p lm-ui language_gallery -- --ignored` renders the
  welcome card, an object card and the Style tab in every language.

**Screens wired** (all of `crates/lm-ui` and the app): first launch and welcome card, toolbar and
search, basemap chooser and credits, layer list and its menus, object card and legend, Style tab,
table drawer and filters, join flow, analysis composer and result card, time slider, drawing,
export sheet, data export, map package, saving and missing files, data library, update notice,
Ask (Pro) chrome, dictation, diagnostics window, screen-reader announcements, and the menu bar
(with View › Language).

**Fonts.** Inter and Noto Sans SC are bundled (Noto Sans SC covers kanji/hanja in Chinese shapes
but no kana-specific forms and no hangul). For Japanese and Korean the app reads the system's own
face at launch and puts it before Noto Sans SC: Hiragino Sans W3/W6 or Apple SD Gothic Neo
(Regular, SemiBold) on a Mac, Yu Gothic UI (Meiryo as a fallback) or Malgun Gothic on Windows
(not yet tried on a Windows machine). **App-size cost: 0 bytes.** Memory: the file stays loaded
(about 16 MB for Hiragino's two weights, 55 MB for Apple SD Gothic Neo's collection, shared by
both weights). Only when the interface is in that language. The alternative, bundling Noto Sans
JP and KR (OFL-1.1) as static subset OTFs in two weights each (Regular and Medium, like Noto
Sans SC), would add 18.4 MB to the app: JP 4.53 + 4.55 MB, KR 4.64 + 4.67 MB (noto-cjk
`Sans/SubsetOTF` at the commit `assets/fonts` already pins);
consider it only if a system face turns out to be missing somewhere. Korean place names in a
user's data still need the Korean interface (or a future always-on fallback) to show hangul.

**What stays English for now** (text built in other crates, outside lm-ui; each needs those
crates to hand over a template plus values instead of a finished sentence):
- insight lines on the object card ("higher than 99% of 4,999 storms", "tied 4th lowest of 17",
  "+0.38 km² above the median"), number and object titles (`lm-jobs` inspect);
- style preset names ("Single color", "By area_km2", "Labels on"), legend titles and labels;
- layer subtitles ("17 areas · districts.geojson", `lm-core` profile), kind nouns ("points",
  "storms") inserted into translated sentences, filter chips, grid captions and dates;
- result cards and recipes from analyses, analysis progress phases and errors (`lm-jobs`
  actions), "Source changed" on kept results;
- undo labels ("Undo Colour by magnitude"; they are also stored in the document's history);
- import, library, saving and loading errors (`lm-jobs`, `lm-doc`), assumption lines;
- the Pro assistant's own messages (`lm-ask`), and model answers;
- action requirement reasons in `rules.toml` (not shown in the interface today);
- dates on the time slider (month names);

Known rough edges for the reviewers: the filter bar is assembled from pieces in English word
order ("Show [storms] where [strength] is any of …"), which reads stiffly in every other language;
the short connector words between composer controls (`and`, `of`, `By`, `With`); sentences that
drop an English noun or a layer name into a gendered or inflected language (fr, es, it, pt, ru,
de); the long German/Dutch/Russian labels in narrow controls (two segmented controls were
shortened after the gallery showed them clipped). Japanese uses the system face whose
baseline sits slightly differently from Inter's in mixed lines.

## 5. Glossary per language

The full term lists (about 175 rows each: every app label quoted on the site, plus the core GIS
words, with notes on where an everyday word was chosen over the QGIS term) are in
[`docs/lm/l10n/terms-<lang>.tsv`](l10n/). The core:

| English | de | es | fr | it | nl | pt-br | ru | ja | ko |
|---|---|---|---|---|---|---|---|---|---|
| Layer | Layer | Capa | Couche | layer | Kaartlaag | Camada | Слой | レイヤー | 레이어 |
| Basemap | Hintergrundkarte | Mapa base | Fond de carte | Mappa di base | Achtergrondkaart | Mapa base | Подложка | 背景地図 | 배경 지도 |
| Attribute table / Table | Attributtabelle | Tabla de atributos | Table attributaire | tabella degli attributi | Attributentabel | Tabela de atributos | Таблица атрибутов | 属性テーブル | 속성 테이블 |
| Field | Feld | Campo | Champ | campo | Veld | Campo | Поле | フィールド | 필드 |
| Feature | Objekt | Elemento | Entité | elemento | Object | Feição | Объект | 地物 | 피처 |
| Point | Punkt | Punto | Point | punto | Punt | Ponto | Точка | 点 | 점 |
| Line | Linie | Línea | Ligne | linea | Lijn | Linha | Линия | 線 | 선 |
| Area (polygon) | Fläche | Área (polígono) | Surface | area (poligono) | Vlak | Área | Область | 面 | 면 |
| Raster | Raster | Ráster | Raster | raster | Raster | Raster | Растр | ラスター | 래스터 |
| Vector data | Vektordaten | Datos vectoriales | Données vectorielles | dati vettoriali | Vectordata | Dados vetoriais | Векторные данные | ベクターデータ | 벡터 데이터 |
| Coordinate system (CRS) | Koordinatensystem (KBS) | Sistema de coordenadas (SRC) | Système de coordonnées (SCR) | Sistema di coordinate (SR) | Coördinatenstelsel (CRS) | Sistema de coordenadas | Система координат (СК) | 座標系 | 좌표계(CRS) |
| Legend | Legende | Leyenda | Légende | Legenda | Legenda | Legenda | Легенда | 凡例 | 범례 |
| Scale bar | Maßstabsleiste | Barra de escala | Barre d’échelle | Barra di scala | Schaalbalk | Barra de escala | Масштабная линейка | スケールバー | 축척 막대 |
| Filter | Filter | Filtrar | Filtre | Filtro | Filter | Filtro | Фильтр | フィルター | 필터 |
| Style | Stil | Estilo | Style | Stile | Stijl | Estilo | Стиль | スタイル | 스타일 |
| Palette | Farbpalette | Paleta | Palette | Tavolozza | Kleurenpalet | Paleta | Палитра | カラーパレット | 팔레트 |
| Join | Verknüpfung | Unión | Jointure | Unione | Koppeling | União | Соединение | 結合 | 조인 |
| Buffer | Puffer | Área de influencia | Zone tampon | buffer | Buffer | Buffer | Буфер | バッファ | 버퍼 |
| Card | Infokarte | Ficha | Fiche | riquadro | Infokaart | Cartão | Карточка | カード | 카드 |
| Examples | Beispiele | Ejemplos | Exemples | Esempi | Voorbeelden | Exemplos | Примеры | サンプル | 예제 |
| Share | Teilen | Compartir | Partager | Condividi | Delen | Compartilhar | Поделиться | 共有 | 공유 |
| Undo | Widerrufen | Deshacer | Annuler | Annulla | Ongedaan maken | Desfazer | Отменить | 取り消す | 실행 취소 |
| Time slider | Zeitschieberegler | Barra de tiempo | Curseur temporel | cursore temporale | Tijdbalk | Controle de tempo | Ползунок времени | タイムスライダー | 시간 슬라이더 |
| No data | Keine Daten | Sin datos | Aucune donnée | Nessun dato | Geen gegevens | Sem dados | Нет данных | データなし | 데이터 없음 |
| Keep | Behalten | Conservar | Conserver | Mantieni | Behouden | Manter | Оставить | 残す | 유지 |
| Ask | Fragen | Preguntar | Demander | Chiedi | Vragen | Perguntar | Спросить | 質問 | 질문하기 |

Choices to confirm first, per language (from the translators' notes):
- **de**: formal "Sie" throughout, app and site (owner's choice, 2026-10-19); "Layer" (QGIS) rather than "Ebene";
  Card = "Infokarte" (plain "Karte" is the map); macOS menu names (Ablage, Darstellung).
- **es**: "tú"; Feature = "elemento" (QGIS: entidad/objeto espacial); Card = "ficha"; numbers as
  the RAE's Ortografía recommends, on the site and in the app alike (owner's choice, 2026-10-19):
  a no-break space between thousands from five digits on (4999, 14 537) and the decimal point
  (35.3 km²); readers in Spain often expect a decimal comma.
- **fr**: Card = « fiche »; Caption = « texte sous la carte » (Légende is the legend); Area =
  « surface » (QGIS: polygone).
- **it**: Card = "riquadro" (scheda is a tab); Help = "Aiuto" but the page "Guida di NimbleMap";
  Light/Dark agree in the masculine.
- **nl**: formal "u" throughout, app and site (owner's choice, 2026-10-19); Card = "infokaart"; Range = "waardebereik" so that Extent can be "bereik";
  two words for Size (formaat on the export sheet, grootte in the Style tab).
- **pt-BR**: Feature = "feição" (QGIS); Join = "união"; Zoom in/out = "aproximar/afastar".
- **ru**: Basemap = «подложка»; Area = «область» (QGIS: полигон); Zoom in/out =
  «приблизить/отдалить»; Glossary = «словарь».
- **ja**: レイヤー/ラスター/ベクター with the long vowel (QGIS writes レイヤ…); 背景地図 rather
  than ベースマップ; 点・線・面; 地図から外す for Remove from map (never 削除).
- **ko**: 점/선/면 rather than 포인트/라인/폴리곤; 배경 지도 rather than 베이스맵; 합니다체.

## 6. Native review checklists

Every language below is machine-translated. Before release, a native speaker who uses maps (ideally
a GIS user) checks each item and signs off here.

**Deutsch** (`de`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**Español** (`es`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**Français** (`fr`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**Italiano** (`it`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**Nederlands** (`nl`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**Português (Brasil)** (`pt-BR`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**Русский** (`ru`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**日本語** (`ja`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**한국어** (`ko`) — reviewer: ______ date: ______
- [ ] Term list (`docs/lm/l10n/terms-<lang>.tsv`): every row, especially the ones whose note says an everyday word was chosen over the QGIS term
- [ ] Website: home, download, help, glossary (page and `glossary-<lang>.js`), three workflows — wording, facts and numbers identical to English, titles and descriptions as people search
- [ ] Website: number and date formats, punctuation (quotes, spaces before punctuation), address form
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in
- [ ] Screenshots in the language retaken once the interface is approved (the pages then quote its labels instead of English + gloss)

**简体中文 (app catalog only; the Chinese pages were written earlier)** (`zh`) — reviewer: ______ date: ______
- [ ] App catalog (`crates/lm-core/src/i18n/<code>.tsv`): every string, read in the running app (`LM_LANGUAGE=<code>`), with an eye on labels that are too long for their buttons
- [ ] App: the action labels from `rules.toml` read naturally with layer names filled in

