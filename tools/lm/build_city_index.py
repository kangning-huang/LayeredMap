#!/usr/bin/env python3
"""WP1.5: build the bundled place-search index, assets/geonames/cities.tsv, from GeoNames.

Downloads cities15000.zip (every place with a population of at least 15,000, or a capital),
admin1CodesASCII.txt and countryInfo.txt from download.geonames.org, and writes one row per city,
sorted by population (largest first; ties by geonameid). Standard library only:

    python3 tools/lm/build_city_index.py [--scratch DIR] [--offline]

--scratch is where the downloads go (default: $TMPDIR/layeredmap-geonames). --offline reuses
files already in it. The script also rewrites assets/geonames/README.md (source date, row count,
SHA-256, attribution). Data from GeoNames (geonames.org), CC BY 4.0.

Columns of cities.tsv (tab-separated, UTF-8, no header, no quoting; tabs and newlines never occur
in the fields):

    geonameid  name  asciiname  zh  admin1  country  country_code  lat  lon  population  feature_code

Chinese name (zh) heuristic. GeoNames' alternatenames column is an untagged list, so the
Simplified-Chinese name is chosen from it:
  1. Candidates are names made only of Han ideographs (CJK Unified Ideographs, Extension A and the
     compatibility block, plus the middle dot). Anything containing kana, hangul or Latin letters
     is dropped, which removes Japanese readings and Korean names.
  2. A candidate that contains a Traditional-only character, or a Japanese shinjitai form that
     Simplified Chinese does not use (both from the small tables below), is used only when no
     clean candidate exists. So 武汉 beats 武漢 and 东京 beats 東京.
  3. In China, Taiwan, Hong Kong and Macau the official form with an administrative suffix
     (市, 区, 县, 镇, 旗, 盟) is preferred (成都市, 上海市, 台北市); elsewhere the bare name is
     preferred over one ending in 市 (大阪, not 大阪市; 京都, not 京都市).
  4. Single-character abbreviations (沪) and names beginning with a country prefix (中国上海)
     lose to anything else.
  5. Then the candidate that shares a prefix with the most other candidates (成都 has 成都市),
     then the shorter, then code-point order, so the result is deterministic.
The column is empty when there is no candidate.
"""
import argparse, csv, datetime, hashlib, io, os, pathlib, sys, tempfile, urllib.request, zipfile

BASE = "https://download.geonames.org/export/dump/"
FILES = ["cities15000.zip", "admin1CodesASCII.txt", "countryInfo.txt"]
REPO = pathlib.Path(__file__).resolve().parents[2]
OUT_DIR = REPO / "assets" / "geonames"

# Traditional-only characters common in place names (each has a different Simplified form).
TRADITIONAL = set(
    "東書車長門開關間陽陰陸陳際階雲電頭題風飛馬驛魚鳥鳳鶴鹽麗黃齊龍國圖圓園團場壩壽夢寧實寶將對"
    "專層屬嶺島峽崗巖廣廳廟張強彎後從徑復應慶戰撫擴數斷時晉會業樂樓標橋樹機檢權歐歲歷歸氣漢溝滿"
    "滬濟濱灣烏無熱爾牆獅獻現琿環產畫當疊發盤盧眾礦祿禮種稱穌窩競節範築簡類糧紅紀約納紐純紹經綠"
    "維網線練縣總繩繼續羅義習聖聯聲肅臨臺與興舊莊華萊葉蓮蔣蕭薩藍蘇蘭處號衛裝見規覺觀許詩語調"
    "謝證譚議護讀變豐貝貢貨貴賀賓賢質賴贛趙軍輝輪農這連進運過達遠遷選遼鄉鄭鄧鄰醫釋銀銅鋼錦錫鎮"
    "鐵閩闊雙雞離難靈韓項順領額館駐體鬥鬱魯鮮鴨鵝麥黨億價傳優儀倫偉僑內兩兒凱劉則創剛劍動勝勞勢"
    "區協單廠參員問啟喬營嚴圍壇壓壯奧學寬審寫導屆岡崙嶽巒幣幫庫廈彥彌徵恆愛態憲懷戲據攤於曉條楊"
    "極榮構樣歡殘沒況淨淺渾測湯溫滄漁潔潛澤濁濕瀋瀘灘為煙爐狀獨獲瑪瓊甌畢異療監碼磯禪稅穀積穩窮"
    "竇籃籠粵紡細終組結絕絲綏綜緊緒緬編縮織繪職膠舉蘆蘋虛蟲補襲覽設訪評識譯貓負財貧責貫購賽贊"
    "跡躍軌軒載輔輕輸轉辦邊郵鄒醬釣鈴鉛鋒錢鍋鐘鑄閣隨險隱雜靜韋響頓顧飯餘驅驚鯉鰲鳴鴻鵬鶯鷹齋"
    "龜灤濰諸嘯潯壢遜兗瀏"  # last group: a few more common in Chinese place names
)
# Japanese shinjitai forms that Simplified Chinese writes differently.
JAPANESE = set("県沢浜広辺栄鉄竜桜関発徳恵営斎歳稲黒滝渋塩亀縄駅円戸児豊")
CN_LIKE = {"CN", "TW", "HK", "MO"}
CN_SUFFIX = tuple("市区县镇旗盟")  # not 州: it ends ordinary city names (杭州, 广州)
COUNTRY_PREFIXES = ("中国", "中華", "中华", "台湾", "臺灣")


def is_han(ch):
    o = ord(ch)
    return 0x4E00 <= o <= 0x9FFF or 0x3400 <= o <= 0x4DBF or 0xF900 <= o <= 0xFAFF or ch == "·"


def choose_zh(altnames, cc):
    cands = sorted({a.strip() for a in altnames.split(",") if a.strip() and all(is_han(c) for c in a.strip())})
    if not cands:
        return ""

    def key(s):
        foreign = any(c in TRADITIONAL or c in JAPANESE for c in s)
        if cc in CN_LIKE:
            suffix_bad = not s.endswith(CN_SUFFIX)
        else:
            suffix_bad = s.endswith("市")
        odd = len(s) == 1 or s.startswith(COUNTRY_PREFIXES)
        support = sum(1 for t in cands if t != s and (t.startswith(s) or s.startswith(t)))
        return (foreign, suffix_bad, odd, -support, len(s), s)

    return min(cands, key=key)


def fetch(scratch, offline):
    scratch.mkdir(parents=True, exist_ok=True)
    for name in FILES:
        path = scratch / name
        if offline and path.exists():
            continue
        req = urllib.request.Request(BASE + name, headers={"User-Agent": "LayeredMap-build/1"})
        with urllib.request.urlopen(req, timeout=120) as r:
            path.write_bytes(r.read())
    with zipfile.ZipFile(scratch / "cities15000.zip") as z:
        info = z.getinfo("cities15000.txt")
        text = z.read(info).decode("utf-8")
    source_date = datetime.date(*info.date_time[:3]).isoformat()
    return text, source_date


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--scratch", default=os.path.join(tempfile.gettempdir(), "layeredmap-geonames"))
    ap.add_argument("--offline", action="store_true")
    args = ap.parse_args()
    scratch = pathlib.Path(args.scratch)
    text, source_date = fetch(scratch, args.offline)

    admin1 = {}
    for line in (scratch / "admin1CodesASCII.txt").read_text(encoding="utf-8").splitlines():
        f = line.split("\t")
        if len(f) >= 2:
            admin1[f[0]] = f[1]
    countries = {}
    for line in (scratch / "countryInfo.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        f = line.split("\t")
        if len(f) >= 5:
            countries[f[0]] = f[4]

    rows = []
    for line in text.splitlines():
        f = line.split("\t")
        if len(f) < 19:
            continue
        gid, name, ascii_, alt, lat, lon, _fclass, fcode, cc, _cc2, a1 = f[:11]
        pop = int(f[14] or 0)
        clean = lambda s: s.replace("\t", " ").replace("\r", " ").replace("\n", " ").strip()
        rows.append((
            int(gid), clean(name), clean(ascii_), choose_zh(alt, cc),
            clean(admin1.get(f"{cc}.{a1}", "")), clean(countries.get(cc, "")), cc,
            f"{float(lat):.5f}", f"{float(lon):.5f}", pop, fcode,
        ))
    rows.sort(key=lambda r: (-r[9], r[0]))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / "cities.tsv"
    body = "".join("\t".join(str(v) for v in r) + "\n" for r in rows)
    out.write_bytes(body.encode("utf-8"))
    sha = hashlib.sha256(body.encode("utf-8")).hexdigest()
    with_zh = sum(1 for r in rows if r[3])
    size = out.stat().st_size

    (OUT_DIR / "README.md").write_text(f"""# Bundled city index (place search)

`cities.tsv` is the offline index behind toolbar place search (WP1.5).

**Data from GeoNames (geonames.org), CC BY 4.0.** <https://www.geonames.org/>,
licence <https://creativecommons.org/licenses/by/4.0/>. Derived from `cities15000.zip`,
`admin1CodesASCII.txt` and `countryInfo.txt` at <{BASE}>. Changes: columns selected, region and
country codes replaced by names, one Simplified-Chinese name chosen from the alternate names
(heuristic documented in `tools/lm/build_city_index.py`), coordinates rounded to 5 decimals,
rows sorted by population.

| | |
|---|---|
| Source file date | {source_date} |
| Built | {datetime.date.today().isoformat()} |
| Rows | {len(rows)} |
| Rows with a Chinese name | {with_zh} |
| Size | {size} bytes |
| SHA-256 of cities.tsv | `{sha}` |

Rebuild:

    python3 tools/lm/build_city_index.py --scratch /path/to/scratch

Columns (tab-separated, UTF-8, no header): geonameid, name, asciiname, zh, admin1 (region),
country, country code, lat, lon, population, feature code.
""", encoding="utf-8")
    print(f"{len(rows)} rows, {with_zh} with zh, {size} bytes, sha256 {sha}, source {source_date}")


if __name__ == "__main__":
    sys.exit(main())
