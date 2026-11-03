#!/usr/bin/env python3
"""Build the big bundled catalogue.

The site's data page is a *curated* list — the three dozen sets the owner thinks are most worth a
hobbyist's evening. The bundled catalogue has a different job: hold **everything a person might
plausibly ask for**, so that a search is answered from the phone instead of from a model or a
search engine. A model costs money and a search engine costs trust; a 2 MB JSON file costs
neither, and works on the Tube.

So this harvests whole families that publish at predictable URLs, rather than hand-picking:

* **All the Places** — 5,700-odd brands, each scraped from that brand's own store finder and
  published as GeoJSON at a stable path. This is the family that answers "Walmart", the search
  that failed in every earlier round: `walmart_us.geojson` is 362 KB of real points.
* **Natural Earth** — the reference layers (countries, states, rivers, roads, railways, airports,
  urban areas, time zones…) at three scales.

Both are public domain or CC0, both are small enough for a phone, and neither needs a key.

Curated entries from `build-catalog.py` are kept and sorted first: they have hand-written blurbs
that no generated title can match.

    python3 scripts/harvest-catalog.py            # rebuild catalog.json
    python3 scripts/harvest-catalog.py --verify 60  # also spot-check 60 random links
"""
import argparse
import json
import pathlib
import random
import re
import subprocess
import sys
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
OUT = HERE.parent / "ios/NimbleMapAltD/catalog.json"
UA = ("Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 "
      "(KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1")

# Spiders below this many places are usually a brand with a handful of shops or a scrape that
# half-failed; they make search noisier without making it more useful.
MIN_PLACES = 25

COUNTRIES = {
    "us": "United States", "ca": "Canada", "mx": "Mexico", "br": "Brazil", "ar": "Argentina",
    "cl": "Chile", "co": "Colombia", "pe": "Peru", "gb": "United Kingdom", "uk": "United Kingdom",
    "ie": "Ireland", "fr": "France", "de": "Germany", "es": "Spain", "pt": "Portugal",
    "it": "Italy", "nl": "Netherlands", "be": "Belgium", "lu": "Luxembourg", "ch": "Switzerland",
    "at": "Austria", "dk": "Denmark", "se": "Sweden", "no": "Norway", "fi": "Finland",
    "is": "Iceland", "pl": "Poland", "cz": "Czechia", "sk": "Slovakia", "hu": "Hungary",
    "ro": "Romania", "bg": "Bulgaria", "gr": "Greece", "hr": "Croatia", "si": "Slovenia",
    "rs": "Serbia", "ua": "Ukraine", "ru": "Russia", "tr": "Turkey", "il": "Israel",
    "ae": "United Arab Emirates", "sa": "Saudi Arabia", "eg": "Egypt", "za": "South Africa",
    "ng": "Nigeria", "ke": "Kenya", "ma": "Morocco", "in": "India", "pk": "Pakistan",
    "cn": "China", "jp": "Japan", "kr": "South Korea", "tw": "Taiwan", "hk": "Hong Kong",
    "sg": "Singapore", "my": "Malaysia", "th": "Thailand", "vn": "Vietnam", "id": "Indonesia",
    "ph": "Philippines", "au": "Australia", "nz": "New Zealand", "ee": "Estonia",
    "lv": "Latvia", "lt": "Lithuania", "by": "Belarus", "md": "Moldova", "ge": "Georgia",
}

# Natural Earth: (folder, layer, title, what it is, keywords)
NE = [
    ("cultural", "admin_0_countries", "Countries of the world", "Every country, with population, economy and region.", ["countries", "borders", "nations", "world", "admin", "boundaries"]),
    ("cultural", "admin_0_boundary_lines_land", "Country borders", "The world's land borders as lines.", ["borders", "boundaries", "frontier", "countries"]),
    ("cultural", "admin_1_states_provinces", "States and provinces", "First-level divisions: states, provinces, regions, prefectures.", ["states", "provinces", "regions", "counties", "admin"]),
    ("cultural", "populated_places", "Towns and cities", "Populated places with population and capital status.", ["cities", "towns", "places", "population", "capitals", "settlements"]),
    ("cultural", "airports", "Airports of the world", "Major airports worldwide.", ["airports", "airfields", "aviation", "flights", "runways"]),
    ("cultural", "ports", "Ports and harbours", "Major ports worldwide.", ["ports", "harbours", "harbors", "shipping", "docks"]),
    ("cultural", "roads", "Major roads", "Motorways and trunk roads worldwide.", ["roads", "highways", "motorways", "driving"]),
    ("cultural", "railroads", "Railways of the world", "Main-line railways worldwide.", ["railways", "railroads", "trains", "rail", "transit"]),
    ("cultural", "urban_areas", "Built-up urban areas", "Where cities actually sprawl, as seen from space.", ["urban", "cities", "built up", "sprawl", "metropolitan"]),
    ("cultural", "time_zones", "Time zones", "The world's time zones as areas.", ["time zones", "timezones", "utc", "clocks"]),
    ("cultural", "parks_and_protected_lands_area", "Parks and protected land", "National parks and protected areas (United States).", ["parks", "protected", "national parks", "wilderness", "nature"]),
    ("physical", "coastline", "Coastlines", "The world's coastlines as lines.", ["coast", "coastline", "shore", "sea", "land"]),
    ("physical", "land", "Land", "All land as areas — the shape of the continents.", ["land", "continents", "landmass"]),
    ("physical", "ocean", "Oceans", "The sea as one area.", ["ocean", "sea", "water", "marine"]),
    ("physical", "rivers_lake_centerlines", "Rivers of the world", "Rivers and lake centre lines.", ["rivers", "streams", "waterways", "hydrology", "water"]),
    ("physical", "lakes", "Lakes", "The world's lakes.", ["lakes", "water", "reservoirs", "hydrology"]),
    ("physical", "glaciated_areas", "Glaciers and ice", "Permanently glaciated areas.", ["glaciers", "ice", "ice sheet", "arctic", "antarctic"]),
    ("physical", "geography_regions_polys", "Named regions", "Deserts, basins, plains and other named physical regions.", ["deserts", "regions", "basins", "plains", "geography"]),
    ("physical", "minor_islands", "Small islands", "Islands too small for the main land layer.", ["islands", "isles", "archipelago", "atolls"]),
    ("physical", "reefs", "Coral reefs", "The world's reefs.", ["reefs", "coral", "marine", "diving"]),
]
SCALES = [("10m", "detailed"), ("50m", "medium detail"), ("110m", "simple")]


# Words people type that an entry answers but never says. The study's reader searched "walmart",
# "bike lanes", "transit" and "shipwrecks" and got nothing, while entries that genuinely answer all
# four sat in the catalogue: the chain-store feed *is* Walmart, and the three OpenStreetMap entries
# *are* bike lanes. An entry is only given a word here if tapping it really does lead to that data
# — this is a synonym list, not advertising.
CURATED_KEYWORDS = {
    "chain-stores-brand-by-brand": [
        "walmart", "starbucks", "mcdonalds", "ikea", "tesco", "aldi", "costco", "target",
        "supermarket", "shops", "stores", "retail", "brands", "chains", "fast food", "petrol",
        "gas stations", "pharmacy", "banks", "atm", "poi", "points of interest",
    ],
    "anything-on-openstreetmap-overpass-turbo": [
        "anything", "custom", "openstreetmap", "osm", "bike", "bicycle", "cycleway", "cycle lanes",
        "bike lanes", "bus", "bus stops", "transit", "tram", "subway", "metro", "stations",
        "benches", "postboxes", "lighthouses", "shipwrecks", "wrecks", "pubs", "cafes", "schools",
        "routes", "cycle routes", "bus routes", "paths", "canals", "bridges", "tunnels",
        "hospitals", "footpaths", "trails", "parks", "playgrounds", "toilets", "fountains",
    ],
    "your-city-from-openstreetmap-bbbike-extracts": [
        "city", "openstreetmap", "osm", "bike", "bicycle", "cycleway", "bike lanes", "cycle lanes",
        "roads", "streets", "footpaths", "pavements", "sidewalks", "buildings", "transit", "bus",
        "railways", "tram", "local", "neighbourhood", "extract", "routes", "paths", "lanes",
    ],
    "a-whole-country-from-openstreetmap-geofabrik": [
        "country", "openstreetmap", "osm", "roads", "railways", "bike", "cycleway", "bike lanes",
        "buildings", "landuse", "waterways", "transit", "boundaries", "places", "extract", "whole",
        "routes", "paths", "lanes", "rivers", "forests",
    ],
    "japan-s-railways-and-stations": [
        "trains", "train", "rail", "railways", "transit", "metro", "subway", "stations", "tracks",
        "shinkansen", "japan", "commuting",
    ],
    "every-airport-airstrip-and-heliport": [
        "airports", "airfields", "aviation", "runways", "heliports", "airstrips", "flying", "planes",
    ],
    "world-port-index": [
        "ports", "harbours", "harbors", "shipping", "maritime", "docks", "anchorage", "cargo", "sea",
    ],
    "where-people-live-1-km-grid": [
        "population", "density", "people", "inhabitants", "demographics", "census", "crowded",
        "where people live", "settlement",
    ],
    "world-cities-with-population": [
        "cities", "towns", "population", "places", "urban", "settlements", "capitals",
    ],
    "new-york-city-s-street-trees": [
        "trees", "street trees", "urban forest", "nyc", "new york", "greenery", "canopy",
    ],
    "earthquakes-live": ["earthquakes", "quakes", "seismic", "tremors", "magnitude", "faults"],
    "fires-burning-right-now": ["fires", "wildfires", "bushfires", "hotspots", "burning", "smoke"],
    "every-us-tornado-since-1950": ["tornadoes", "twisters", "storms", "severe weather", "wind"],
    "paths-of-the-2023-and-2024-solar-eclipses": ["eclipse", "eclipses", "solar", "totality", "shadow"],
    "every-known-meteorite": ["meteorites", "meteors", "space rocks", "impacts", "craters"],
    "earth-at-night": ["night lights", "lights", "nighttime", "satellite", "electricity", "dark"],
    "global-forest-loss-2001-2025": ["deforestation", "forest", "trees", "logging", "canopy loss"],
    "roads-of-the-roman-empire": ["roman", "romans", "roads", "empire", "ancient", "history", "viae"],
    "pleiades-places-of-the-ancient-world": ["ancient", "classical", "antiquity", "greek", "roman", "archaeology"],
    "china-s-county-seats-221-bce-to-1911": ["china", "chinese", "historical", "counties", "dynasties", "imperial"],
    "buddhist-temples-of-china": ["temples", "buddhist", "buddhism", "religion", "china", "monasteries"],
    "castles-from-wikidata": ["castles", "forts", "fortresses", "palaces", "ruins", "medieval", "history"],
    "unesco-world-heritage-sites": ["unesco", "heritage", "world heritage", "monuments", "landmarks"],
    "white-stork-migrations-1991-2017": ["birds", "storks", "migration", "wildlife", "tracking", "animals"],
    "solar-farms-seen-from-space": ["solar", "renewables", "panels", "energy", "photovoltaic"],
    "every-wind-turbine-in-the-united-states": ["wind", "turbines", "renewables", "energy", "windfarms"],
    "nuclear-power-reactors-of-the-world": ["nuclear", "reactors", "power", "energy", "atomic"],
    "the-world-s-mines-outlined": ["mines", "mining", "quarries", "extraction", "minerals", "pits"],
    "sea-surface-temperature-since-1854": ["sea", "ocean", "temperature", "sst", "climate", "warming"],
    "rain-and-snow-everywhere-since-1979": ["rain", "rainfall", "snow", "precipitation", "climate", "weather"],
    "life-expectancy-every-country": ["life expectancy", "health", "mortality", "demographics", "lifespan"],
    "singapore-s-hawker-centres": ["singapore", "hawker", "food", "markets", "street food", "stalls"],
}


def title_case(words):
    small = {"and", "of", "the", "de", "la", "du", "van", "der"}
    out = []
    for i, w in enumerate(words):
        out.append(w if (w.isupper() and len(w) <= 3) else
                   (w.lower() if i and w.lower() in small else w.capitalize()))
    return " ".join(out)


def probe_spiders(spiders):
    """Which of these still serve a usable file, asked in parallel."""
    import concurrent.futures

    def one(spider):
        url = f"https://data.alltheplaces.xyz/runs/latest/output/{spider}.geojson"
        r = subprocess.run(
            ["curl", "-sIL", "-A", UA, "--max-time", "20", "-o", "/dev/null",
             "-w", "%{http_code}\t%{content_type}\t%{size_download}\t%{size_header}"],
            capture_output=True, text=True)
        r = subprocess.run(
            ["curl", "-sIL", "-A", UA, "--max-time", "20", "-o", "/dev/null",
             "-w", "%{http_code}\t%{content_type}\t%{header_json}", url],
            capture_output=True, text=True)
        code, ctype, headers = (r.stdout.split("\t") + ["", "", ""])[:3]
        if not code.startswith("2") or "geo+json" not in ctype:
            return None
        try:
            length = int(json.loads(headers).get("content-length", ["0"])[-1])
        except Exception:  # noqa: BLE001
            length = 0
        # An empty FeatureCollection is a couple of hundred bytes; anything real is far bigger.
        return spider if length > 2_000 else None

    with concurrent.futures.ThreadPoolExecutor(16) as ex:
        return {s for s in ex.map(one, spiders) if s}


def from_all_the_places():
    def get(url, timeout):
        # The CDN in front of these files rejects urllib's default user agent with a 403.
        req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.load(r)

    try:
        run = get("https://data.alltheplaces.xyz/runs/latest.json", 40)
        results = get(run["stats_url"], 120)["results"]
    except Exception as e:  # noqa: BLE001
        print(f"all the places: skipped ({e})", file=sys.stderr)
        return []
    # A spider that produced nothing in the latest run has usually just broken for a week — the
    # CDN still serves its last good output from the stable `latest/output/` path. 668 of 5,708
    # reported zero in the run used to write this, and `walmart_us` was one of them while still
    # serving 354 KB: the exact brand that failed every earlier round of this study would have
    # been dropped by trusting the count. So the count decides on its own only when it is healthy;
    # the rest are asked directly.
    healthy = {r["spider"] for r in results if r.get("features", 0) >= MIN_PLACES}
    doubtful = [r["spider"] for r in results if r.get("spider") and r["spider"] not in healthy]
    print(f"all the places: {len(healthy)} healthy, probing {len(doubtful)} that reported nothing…")
    alive = probe_spiders(doubtful)
    print(f"all the places: {len(alive)} of those still serve a file")
    keep = healthy | alive

    items = []
    for row in results:
        spider, n = row.get("spider", ""), row.get("features", 0)
        if spider not in keep:
            continue
        parts = spider.split("_")
        country = ""
        if len(parts) > 1 and parts[-1] in COUNTRIES:
            country = COUNTRIES[parts[-1]]
            parts = parts[:-1]
        brand = title_case(parts)
        if not brand:
            continue
        where = f" ({country})" if country else ""
        items.append({
            "id": f"atp-{spider}",
            "title": f"{brand}{where}",
            # A probe-rescued spider reported nothing this run while still serving its last good
            # file, so its count is a lie — say nothing rather than "0 Walmart locations".
            "blurb": (f"{n:,} {brand} locations, collected from the brand's own store finder."
                      if n >= MIN_PLACES else
                      f"{brand} locations, collected from the brand's own store finder."),
            "source": "All the Places",
            "format": "GeoJSON",
            "size": "",
            "licence": "CC0 — public domain",
            "section": "Everyday places",
            "url": f"https://data.alltheplaces.xyz/runs/latest/output/{spider}.geojson",
            "kind": "file",
            "keywords": sorted({*(p.lower() for p in parts), *(country.lower().split()),
                                "shops", "stores", "locations", "branches", "chain", "brand", "places"}),
        })
    print(f"all the places: {len(items)} brands (of {len(results)} spiders, min {MIN_PLACES} places)")
    return items


def from_natural_earth():
    items = []
    for folder, layer, title, blurb, keys in NE:
        for scale, detail in SCALES:
            items.append({
                "id": f"ne-{scale}-{layer}",
                "title": f"{title}" + ("" if scale == "10m" else f" · {detail}"),
                "blurb": blurb + (" The most detailed version." if scale == "10m" else f" A {detail} version, quicker to open."),
                "source": "Natural Earth",
                "format": "Shapefile (zip)",
                "size": "",
                "licence": "Public domain",
                "section": "Cities & people" if folder == "cultural" else "Nature & wildlife",
                "url": f"https://naciscdn.org/naturalearth/{scale}/{folder}/ne_{scale}_{layer}.zip",
                "kind": "file",
                "keywords": sorted({*keys, "natural earth", "reference", "basemap", "world"}),
            })
    print(f"natural earth: {len(items)} layers")
    return items


def verify(items, n):
    sample = random.Random(7).sample(items, min(n, len(items)))
    bad = []
    for it in sample:
        r = subprocess.run(["curl", "-sIL", "-A", UA, "--max-time", "25", "-o", "/dev/null",
                            "-w", "%{http_code} %{content_type} %{size_download}"],
                           capture_output=True, text=True, input="")
        r = subprocess.run(["curl", "-sIL", "-A", UA, "--max-time", "25", "-o", "/dev/null",
                            "-w", "%{http_code}\t%{content_type}", it["url"]],
                           capture_output=True, text=True)
        code, ctype = (r.stdout.split("\t") + ["", ""])[:2]
        ok = code.startswith(("2", "3")) and "text/html" not in ctype
        if not ok:
            bad.append((it["title"], code, ctype))
        print(("OK  " if ok else "BAD "), code, ctype[:28].ljust(28), it["title"][:44])
    print(f"\nspot check: {len(sample) - len(bad)}/{len(sample)} good")
    for t, c, ct in bad:
        print(f"  bad: {t} ({c} {ct})")
    return len(bad)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--verify", type=int, default=0, help="spot-check this many random links")
    ap.add_argument("--keywords-only", action="store_true",
                    help="re-apply CURATED_KEYWORDS to the existing file and stop")
    args = ap.parse_args()

    everything = json.loads(OUT.read_text()) if OUT.exists() else []
    if args.keywords_only:
        n = 0
        for item in everything:
            words = CURATED_KEYWORDS.get(item["id"])
            if words:
                item["keywords"] = sorted(set(words))
                n += 1
        OUT.write_text(json.dumps(everything, ensure_ascii=False, separators=(",", ":")) + "\n")
        print(f"keywords re-applied to {n} of {len(everything)} entries")
        return
    curated = [i for i in everything if not i["id"].startswith(("atp-", "ne-"))]
    named = 0
    for item in curated:
        words = CURATED_KEYWORDS.get(item["id"])
        if words:
            item["keywords"] = sorted(set(words))
            named += 1
    print(f"curated (kept): {len(curated)}, {named} with keywords")
    missing = [i["id"] for i in curated if not i.get("keywords")]
    if missing:
        print(f"  no keywords yet: {', '.join(missing)}")

    items = curated + from_natural_earth() + from_all_the_places()
    seen, unique = set(), []
    for i in items:
        if i["url"] in seen:
            continue
        seen.add(i["url"])
        unique.append(i)

    OUT.write_text(json.dumps(unique, ensure_ascii=False, separators=(",", ":")) + "\n")
    kb = OUT.stat().st_size / 1024
    print(f"\n{len(unique)} datasets -> {OUT} ({kb:.0f} KB)")
    if args.verify:
        verify(unique, args.verify)


if __name__ == "__main__":
    main()
