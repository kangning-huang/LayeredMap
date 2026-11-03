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


# Entries that answer "where is X" for a place name, and the bundled files their names come from.
#
# Someone typing "Poland" or "Osaka" wants that place on a map. Before this they got 92 Polish
# supermarkets, because the only entries carrying the word were shop branches. The app already
# ships 177 country outlines and 1,251 towns; teaching the catalogue their names costs a few
# hundred kilobytes of keywords and turns every place name into a real answer.
PLACE_NAME_SOURCES = {
    "countries": ("../../../assets/examples/countries.geojson",
                  ["ne-10m-admin_0_countries", "ne-50m-admin_0_countries", "ne-110m-admin_0_countries"]),
    "cities": ("../../../assets/examples/cities.geojson",
               ["ne-10m-populated_places", "ne-50m-populated_places", "ne-110m-populated_places"]),
}


# ---------------------------------------------------------------------------
# Choosers
# ---------------------------------------------------------------------------
#
# Four rows promised a choice and delivered one hardcoded file. "Where people live" said "any
# country and any year from 2000 to 2020" and handed over China, 2020. "Chain stores" said
# "Starbucks, McDonald's, IKEA and thousands more" and handed over Starbucks in the US. "Forest
# loss" said "download the lossyear tile for your region" — there is no *your* in a fixed URL;
# everyone got southern China. "Earth at night" offered three resolutions and two years and gave
# one of the six. Each row read like a menu and behaved like a vending machine with one button.
#
# A chooser is the menu made real: a URL template plus the axes to fill it. Every option is a
# combination this script has actually resolved, so nothing on offer 404s. The size is still asked
# for over the network at the moment of adding, as for any other row — these files run from 4 MB to
# half a gigabyte and the number has to be the real one, not a guess baked in months ago.
#
# Two little template languages, each used in exactly one field: `url` substitutes an option's
# *value*, `naming` substitutes its *label*. So the Hansen URL gets `00N_060W` and the layer on the
# map gets called "Amazon — lower Amazon, Brazil".

# WorldPop publishes one GeoTIFF per country per year at a wholly predictable path. The country
# list is whatever the 2020 directory holds, intersected with the names the app already ships, so
# the picker reads "Kenya" rather than "KEN".
WORLDPOP_DIR = "https://data.worldpop.org/GIS/Population/Global_2000_2020_1km_UNadj/"

# Hansen's lossyear tiles are 10° squares named for their north-west corner, and only the 280 that
# contain land exist. Most people do not think in tile corners, so the ones that matter carry the
# name of the forest instead; the rest keep their coordinates and are still searchable.
HANSEN_RUN = "GFC-2025-v1.13"
HANSEN_NAMES = {
    "00N_050W": "Amazon — the mouth, Brazil",
    "00N_060W": "Amazon — lower Amazon, Brazil",
    "00N_070W": "Amazon — central, Brazil & Colombia",
    "10S_060W": "Amazon — the arc of deforestation, Rondônia",
    "10S_070W": "Amazon — Acre & Madre de Dios, Brazil & Peru",
    "10S_050W": "Amazon — Mato Grosso, Brazil",
    "20S_050W": "Brazil — Cerrado & Atlantic Forest",
    "10N_080W": "Panama, Colombia & the Darién",
    "20N_100W": "Mexico",
    "30N_090W": "United States — Gulf coast pine belt",
    "40N_080W": "United States — the Appalachians",
    "50N_100W": "Canada — the prairie forest edge",
    "60N_100W": "Canada — central boreal",
    "60N_120W": "Canada — western boreal & Alaska",
    "00N_010E": "Congo Basin — west",
    "00N_020E": "Congo Basin — centre",
    "00N_030E": "Congo Basin — east",
    "10S_030E": "Miombo woodlands, Tanzania & Zambia",
    "20S_040E": "Madagascar",
    "10N_000E": "West Africa — Ghana, Côte d'Ivoire & Nigeria",
    "50N_000E": "Western Europe",
    "60N_020E": "Fennoscandia",
    "60N_060E": "Siberian taiga — west",
    "60N_100E": "Siberian taiga — central",
    "50N_120E": "Russian Far East & north-east China",
    "20N_070E": "India — the Western Ghats",
    "30N_110E": "Southern China",
    "00N_100E": "Sumatra",
    "00N_110E": "Borneo",
    "10N_100E": "Mainland South-East Asia",
    "10S_140E": "New Guinea",
    "30S_150E": "South-east Australia",
    "40S_070W": "Patagonia & the Chilean forests",
}

# NASA's Black Marble, as it actually exists: one 2016 composite, whole-world at two resolutions
# and 500 m in eight quarter-hemisphere panels. The old blurb also promised 2012, which is not
# published as a georeferenced GeoTIFF anywhere this script could find, so the row no longer
# claims it.
BLACK_MARBLE_RECORD = ("https://assets.science.nasa.gov/content/dam/science/esd/eo/images/"
                       "imagerecords/144000/144898/BlackMarble_2016_{part}_geo.tif")
# The label is what the layer on the map ends up called, so the size does not belong in it —
# "Earth at night — Whole world, 0.1° — about 4 MB" is a filename, not a name. The size goes in
# the note, where it belongs: it is what you choose *by*, not what you end up with.
BLACK_MARBLE_PARTS = [
    ("01deg", "Whole world, 0.1°"),
    ("3km", "Whole world, 3 km"),
    ("A1", "500 m — western North America, Alaska & the Pacific"),
    ("B1", "500 m — eastern North America, Europe & West Africa"),
    ("C1", "500 m — eastern Europe, the Middle East, Russia & India"),
    ("D1", "500 m — East & South-East Asia"),
    ("A2", "500 m — the South Pacific"),
    ("B2", "500 m — South America"),
    ("C2", "500 m — southern Africa & the Indian Ocean"),
    ("D2", "500 m — Australia & New Zealand"),
]


def http_text(url, timeout=60):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read().decode("utf-8", "replace")


def tile_corner_label(tile):
    """`00N_060W` -> `0–10°S, 60–50°W`: the square it actually covers, not its corner."""
    m = re.fullmatch(r"(\d+)([NS])_(\d+)([EW])", tile)
    if not m:
        return tile
    lat, ns, lon, ew = int(m.group(1)), m.group(2), int(m.group(3)), m.group(4)
    north = lat if ns == "N" else -lat
    west = lon if ew == "E" else -lon
    south, east = north - 10, west + 10

    def lat_text(a, b):
        lo, hi = sorted((a, b))
        if lo >= 0:
            return f"{lo}–{hi}°N"
        if hi <= 0:
            return f"{-hi}–{-lo}°S"
        return f"{-lo}°S–{hi}°N"

    def lon_text(a, b):
        lo, hi = sorted((a, b))
        if lo >= 0:
            return f"{lo}–{hi}°E"
        if hi <= 0:
            return f"{-hi}–{-lo}°W"
        return f"{-lo}°W–{hi}°E"

    return f"{lat_text(north, south)}, {lon_text(west, east)}"


def worldpop_chooser():
    """Country × year, from WorldPop's own directory listing."""
    try:
        listing = http_text(WORLDPOP_DIR + "2020/")
    except Exception as e:  # noqa: BLE001
        print(f"worldpop: skipped ({e})", file=sys.stderr)
        return None
    have = set(re.findall(r'href="([A-Z]{3})/?"', listing))
    names = {}
    path = (HERE / "../../../assets/examples/countries.geojson").resolve()
    try:
        for f in json.loads(path.read_text()).get("features", []):
            p = f.get("properties") or {}
            if p.get("iso_a3") in have and isinstance(p.get("name"), str):
                names[p["iso_a3"]] = p["name"]
    except Exception as e:  # noqa: BLE001
        print(f"worldpop: no country names ({e})", file=sys.stderr)
        return None
    countries = [{"label": n, "value": c} for c, n in sorted(names.items(), key=lambda kv: kv[1])]
    print(f"worldpop: {len(countries)} countries × 21 years")
    return {
        "url": WORLDPOP_DIR + "{year}/{iso3}/{iso3|lower}_ppp_{year}_1km_Aggregated_UNadj.tif",
        "naming": "People per km² — {iso3}, {year}",
        "axes": [
            {"key": "iso3", "label": "Country", "default": "CHN", "options": countries},
            {"key": "year", "label": "Year", "default": "2020",
             "options": [{"label": str(y), "value": str(y)} for y in range(2020, 1999, -1)]},
        ],
    }


def hansen_chooser():
    """Every lossyear tile that exists, named after its forest where it has one."""
    url = (f"https://storage.googleapis.com/earthenginepartners-hansen"
           f"?prefix={HANSEN_RUN}/Hansen_{HANSEN_RUN}_lossyear&max-keys=2000")
    try:
        xml = http_text(url)
    except Exception as e:  # noqa: BLE001
        print(f"hansen: skipped ({e})", file=sys.stderr)
        return None
    tiles = sorted(set(re.findall(r"lossyear_(\d+[NS]_\d+[EW])\.tif", xml)))
    if not tiles:
        print("hansen: no tiles in the bucket listing", file=sys.stderr)
        return None
    unknown = sorted(set(HANSEN_NAMES) - set(tiles))
    if unknown:
        print(f"hansen: {len(unknown)} named tiles no longer exist: {', '.join(unknown)}",
              file=sys.stderr)
    named = [t for t in tiles if t in HANSEN_NAMES]
    rest = [t for t in tiles if t not in HANSEN_NAMES]
    options = ([{"label": HANSEN_NAMES[t], "value": t, "note": tile_corner_label(t)} for t in named]
               + [{"label": tile_corner_label(t), "value": t} for t in rest])
    print(f"hansen: {len(tiles)} tiles, {len(named)} named")
    return {
        "url": (f"https://storage.googleapis.com/earthenginepartners-hansen/{HANSEN_RUN}/"
                f"Hansen_{HANSEN_RUN}_lossyear_{{tile}}.tif"),
        "naming": "Forest loss — {tile}",
        # Every tile is 10° of latitude and longitude, so the one under the map can be worked out
        # rather than hunted for.
        "fromView": {"key": "tile", "grid": 10, "label": "Where I'm looking"},
        "axes": [{"key": "tile", "label": "Area", "default": "00N_060W", "options": options}],
    }


def black_marble_chooser():
    """Ten parts, each with the size it actually is right now.

    These run from 4 MB to half a gigabyte and the choice is mostly *about* the size, so the
    number is measured here rather than written from memory — one HEAD each, at build time.
    """
    import concurrent.futures

    def measure(part):
        r = subprocess.run(["curl", "-sIL", "-A", UA, "--max-time", "25", "-o", "/dev/null",
                            "-w", "%{http_code}\t%{size_header}\t%{header_json}",
                            BLACK_MARBLE_RECORD.format(part=part)],
                           capture_output=True, text=True)
        code, _, headers = (r.stdout.split("\t") + ["", "", ""])[:3]
        if not code.startswith("2"):
            return part, ""
        try:
            n = int(json.loads(headers).get("content-length", ["0"])[-1])
        except Exception:  # noqa: BLE001
            return part, ""
        return part, f"about {round(n / 1_000_000)} MB" if n else ""

    with concurrent.futures.ThreadPoolExecutor(10) as ex:
        sizes = dict(ex.map(measure, [v for v, _ in BLACK_MARBLE_PARTS]))
    missing = [v for v, _ in BLACK_MARBLE_PARTS if not sizes.get(v)]
    if missing:
        print(f"black marble: no size for {', '.join(missing)}", file=sys.stderr)
    print(f"black marble: {len(BLACK_MARBLE_PARTS)} parts, {len(BLACK_MARBLE_PARTS) - len(missing)} sized")
    return {
        "url": BLACK_MARBLE_RECORD,
        "naming": "Earth at night — {part}",
        "axes": [{"key": "part", "label": "Which part, and how fine", "default": "3km",
                  "options": [{"label": lbl, "value": v, "note": sizes.get(v, "")}
                              for v, lbl in BLACK_MARBLE_PARTS]}],
    }


def chain_store_chooser():
    """The brand list is already in this file: 5,600-odd entries the app can search."""
    return {
        "url": "{brand}",
        "naming": "{brand}",
        # Filled from the catalogue's own All the Places entries when the sheet opens, rather than
        # written out a second time — they are a third of the file as it is.
        #
        # The default matters more here than on the other three. Left to fall through to the
        # alphabetically first option the sheet opened on "3 Brasseurs (France)", which reads like
        # a list that has broken rather than one that is waiting for you. Starbucks is the brand
        # the blurb names first and the one the earlier fixed URL used.
        "axes": [{"key": "brand", "label": "Brand", "source": "alltheplaces",
                  "default": "https://data.alltheplaces.xyz/runs/latest/output/starbucks_us.geojson"}],
    }


# The `url` on a chooser row is vestigial — the chooser builds its own from the axes — so it is
# pointed at the publisher's own page instead of at one of the files.
#
# That is not tidiness. "Chain stores" kept the hardcoded `starbucks_us.geojson`, and because
# curated entries are deduplicated first, that one URL *evicted* All the Places' own Starbucks (US)
# entry from the catalogue entirely. The row that was supposed to offer every brand was quietly
# deleting one of them, and the chooser's Starbucks default then had nothing to match and silently
# opened on "3 Brasseurs (France)".
CHOOSERS = {
    "where-people-live-1-km-grid": (worldpop_chooser, {
        "blurb": ("Estimated number of people in every 1 km cell. Choose any country and any year "
                  "from 2000 to 2020 — try China either side of the Hu Line."),
        "size": "",
        "url": "https://hub.worldpop.org/",
    }),
    "global-forest-loss-2001-2025": (hansen_chooser, {
        "blurb": ("A 30-metre map of where forest was cleared, each pixel coded with the year it "
                  "was lost. Choose a forest — the Amazon, the Congo Basin, Borneo — or the tile "
                  "under wherever the map is pointing, then click a pixel to read the year."),
        "size": "",
        "url": "https://glad.earthengine.app/view/global-forest-change",
    }),
    "earth-at-night": (black_marble_chooser, {
        "blurb": ("The planet's city lights in 2016. Choose the whole world at 0.1° or 3 km, or "
                  "one eighth of it at 500 m. Put the bundled cities on top and see which glow "
                  "brightest."),
        "size": "",
        "url": "https://earthobservatory.nasa.gov/features/NightLights",
    }),
    "chain-stores-brand-by-brand": (chain_store_chooser, {
        "blurb": ("Store and branch locations collected every week from the brands' own store "
                  "finders. Choose from several thousand: Starbucks, McDonald's, IKEA, Tesco, "
                  "Aldi — most with a worldwide file and one per country."),
        "size": "",
        "url": "https://www.alltheplaces.xyz/",
    }),
}


def attach_choosers(by_id):
    """Turn the four fixed-URL rows into real pickers.

    Runs *before* deduplication, so that giving these rows back their publisher's page frees the
    data URLs they were holding.
    """
    for entry_id, (build, patch) in CHOOSERS.items():
        item = by_id.get(entry_id)
        if not item:
            print(f"chooser: no entry {entry_id}", file=sys.stderr)
            continue
        chooser = build()
        if not chooser:
            continue
        item.update(patch)
        item["kind"] = "choose"
        item["choose"] = chooser


def place_names(relative):
    """Every place name in a bundled example file, lowercased."""
    path = (HERE / relative).resolve()
    try:
        data = json.loads(path.read_text())
    except Exception as e:  # noqa: BLE001
        print(f"place names: skipped {path.name} ({e})", file=sys.stderr)
        return []
    out = set()
    for f in data.get("features", []):
        name = (f.get("properties") or {}).get("name")
        if isinstance(name, str) and len(name) > 2:
            out.add(name.lower())
    return sorted(out)


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
    attach_choosers({i["id"]: i for i in items})
    seen, unique = set(), []
    for i in items:
        if i["url"] in seen:
            continue
        seen.add(i["url"])
        unique.append(i)

    # Teach the reference layers every place name they can actually show.
    by_id = {i["id"]: i for i in unique}
    for kind, (source, ids) in PLACE_NAME_SOURCES.items():
        names = place_names(source)
        if not names:
            continue
        hit = 0
        for entry_id in ids:
            item = by_id.get(entry_id)
            if not item:
                continue
            item["keywords"] = sorted(set(item.get("keywords", [])) | set(names))
            hit += 1
        print(f"place names: {len(names)} {kind} added to {hit} entries")

    # Downloading a whole country from OpenStreetMap is never the right first move: the row says
    # "every country and many regions" and hands over 263 MB of Taiwan. Send people to the chooser.
    geofabrik = by_id.get("a-whole-country-from-openstreetmap-geofabrik")
    if geofabrik:
        geofabrik["kind"] = "page"
        geofabrik["url"] = "https://download.geofabrik.de/"
        geofabrik["size"] = ""
        geofabrik["title"] = "A country or region from OpenStreetMap"
        geofabrik["blurb"] = ("Pick a country or region and download it. These are whole-country "
                              "extracts and can be hundreds of megabytes, so choose the smallest "
                              "area that covers what you need.")

    OUT.write_text(json.dumps(unique, ensure_ascii=False, separators=(",", ":")) + "\n")
    kb = OUT.stat().st_size / 1024
    print(f"\n{len(unique)} datasets -> {OUT} ({kb:.0f} KB)")
    if args.verify:
        verify(unique, args.verify)


if __name__ == "__main__":
    main()
