"""MapEval-API → sources/mapeval_api.jsonl (own words only).

Source: MapEval/MapEval-API, ``dataset.json`` (300 multiple-choice questions), commit
35d481ac797366592dd20f74bfbb75efe2961dc4 (2024-11-26). Licence: none (the repository has no
LICENSE file), so no question text is copied into git. Each record's summary is generated
from the item's classification and a few keywords (intent, travel mode, number of options);
each case's request is an ORIGINAL question we wrote for the same question type, with place
names we chose. The raw data stays outside git:

    python -I tools/lm/benchmarks/convert/mapeval_api.py [/path/to/MapEval-API]

MapEval-API questions are answered from Google Maps place details, nearby search, directions
and travel times. LayeredMap has none of these (no place database, no road routing, no
opening hours), so every item is out of scope; the class decides the reason.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, case  # noqa: E402

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/Volumes/LaCie/data/layeredmap-benchmarks/mapeval-api/src")
OUT = HERE.parent / "sources/mapeval_api.jsonl"
SOURCE = "MapEval-API"
LICENCE = "none (no LICENSE file)"

CLASS = {
    # classification: (category, oos_reason)
    "nearby": ("proximity", "geocoding_service"),
    "trip": ("network", "live_routing"),
    "routing": ("network", "live_routing"),
    "poi": ("data_acquisition", "internet_data"),
    "None": ("geocoding", "geocoding_service"),
}

# Original requests, ours. (text, lang, context)
REQUESTS = {
    "nearby": [
        ("I'm at the Rijksmuseum. Which café is closest to me?", "en", "world"),
        ("What is the nearest bakery to Shinjuku Station?", "en", "world"),
        ("I'm standing outside the Colosseum. Where's the closest pharmacy?", "en", "world"),
        ("Find the second closest bookshop to Plaza Mayor in Madrid.", "en", "world"),
        ("I'm near Union Square in San Francisco and starving. Where can I grab a quick bite?", "en", "usa"),
        ("Which supermarket near Kings Cross is open after 10 pm on a Sunday?", "en", "world"),
        ("Suggest a ramen place near Sapporo Station rated at least 4.3.", "en", "world"),
        ("How many ATMs are within a five-minute walk of Grand Central?", "en", "usa"),
        ("我在外滩，附近最近的咖啡馆是哪家？", "zh", "shanghai"),
        ("离静安寺最近的地铁站入口在哪里？", "zh", "shanghai"),
        ("我在城隍庙，附近有没有评分4.5以上的小笼包店？", "zh", "shanghai"),
        ("What's the closest hospital emergency room to the Space Needle?", "en", "usa"),
        ("I'm at the Acropolis Museum. Recommend a nearby taverna that serves lunch on Mondays.", "en", "world"),
        ("Which petrol station is nearest to Cape Town's V&A Waterfront?", "en", "world"),
        ("Where is the closest public library to Copley Square?", "en", "usa"),
        ("Find a vegetarian restaurant within walking distance of Charles Bridge.", "en", "world"),
        ("我在故宫东门，最近的公共厕所在哪？", "zh", "world"),
        ("Is there a laundromat near the Plaza de Armas in Lima?", "en", "world"),
    ],
    "trip": [
        ("I'm staying near Waterloo station and want to see the Tate Modern for 2 hours, Borough Market for 1 hour and the British Museum for 2 hours, starting at 9 am on foot. What order wastes the least time?", "en", "world"),
        ("Starting from my hotel by Shibuya Crossing at 10:00, I'll spend 1.5 hours at Meiji Shrine and 2 hours in Asakusa by train. When will I be done?", "en", "world"),
        ("I'll cycle from Amsterdam Centraal to the Vondelpark and walk back. How long is the round trip?", "en", "world"),
        ("Plan the fastest order to visit the Sagrada Família, Park Güell and Casa Batlló by taxi from Plaça de Catalunya.", "en", "world"),
        ("If I leave Denver at 7 am and spend 3 hours in Boulder and 2 in Golden, driving, when do I get back?", "en", "usa"),
        ("我住在南京路步行街附近，想去豫园玩2小时、外滩1小时、陆家嘴2小时，坐地铁的话怎么安排最省时间？", "zh", "shanghai"),
        ("从人民广场出发，开车去朱家角待3小时再回来，总共要多久？", "zh", "shanghai"),
        ("I have 5 hours in Lisbon from Rossio. Can I fit in Belém Tower, Jerónimos Monastery and the LX Factory by tram?", "en", "world"),
        ("Order these stops for a driving day in Chicago from the Loop: Navy Pier, Wrigley Field, Museum Campus.", "en", "usa"),
        ("I'll walk from my hostel in Kreuzberg to the East Side Gallery, then take the U-Bahn to the Reichstag. What time will I arrive if I leave at 2 pm?", "en", "world"),
        ("Leaving Edinburgh Waverley at 8:30, visiting the castle for 2 hours and Arthur's Seat for 2 hours on foot — when do I finish?", "en", "world"),
        ("从西湖断桥出发，骑车去灵隐寺玩2小时，再去河坊街，一共大概几个小时？", "zh", "world"),
        ("What's the quickest order to visit Central Park, the Met and the High Line by subway, starting from Times Square?", "en", "usa"),
        ("I'm driving from Calgary to Lake Louise and Moraine Lake, 2 hours at each, starting at 8 am. When can I be back?", "en", "world"),
        ("From my riad in the medina, how long to walk to Jardin Majorelle and then take a taxi to the Menara Gardens?", "en", "world"),
    ],
    "routing": [
        ("I'm driving from the Louvre to Montmartre along Rue La Fayette. After Boulevard Haussmann, where do I turn?", "en", "world"),
        ("Walking from Hyde Park Corner to Buckingham Palace via Constitution Hill — which way do I turn at the end?", "en", "world"),
        ("How many toll roads do I pass driving from San Jose to Oakland on I-880?", "en", "usa"),
        ("Which exit should I take at the roundabout when driving from the Arc de Triomphe toward La Défense?", "en", "world"),
        ("Which of these is quickest to reach by public transport from Bondi Junction: Manly, Circular Quay or Newtown?", "en", "world"),
        ("从虹桥火车站开车去外滩，走延安高架的话下了高架往哪拐？", "zh", "shanghai"),
        ("步行从人民广场去南京东路，第一个路口要左转还是右转？", "zh", "shanghai"),
        ("Driving from Miami Beach to the airport on the Julia Tuttle Causeway, which highway do I merge onto next?", "en", "usa"),
        ("How long does the bus take from Kowloon Tong to Mong Kok right now?", "en", "world"),
        ("What's the walking route from the Duomo to Ponte Vecchio, and how many streets do I cross?", "en", "world"),
        ("By bike from the Tiergarten to Alexanderplatz along Unter den Linden, what comes after the Brandenburg Gate?", "en", "world"),
        ("从北京南站坐地铁去天安门，需要换乘几次？", "zh", "world"),
        ("Which is faster at rush hour from Midtown to JFK: the AirTrain via Jamaica or a taxi over the Queensboro Bridge?", "en", "usa"),
        ("Driving out of Seattle on I-5 south to Tacoma, which exit do I take for the Museum of Glass?", "en", "usa"),
        ("Does the fastest driving route from Bonn to Cologne use the A555 or the A59?", "en", "world"),
    ],
    "poi": [
        ("Is the Prado Museum open on Monday mornings?", "en", "world"),
        ("Can I book a table at Noma online?", "en", "world"),
        ("Which two of these Roman landmarks are farthest apart: the Pantheon, the Vatican Museums or the Baths of Caracalla?", "en", "world"),
        ("What landmark lies directly east of the Brandenburg Gate?", "en", "world"),
        ("Which café sits between St Paul's Cathedral and the Millennium Bridge?", "en", "world"),
        ("What's the Google rating of the Ferry Building Marketplace?", "en", "usa"),
        ("Does the Field Museum have wheelchair-accessible entrances?", "en", "usa"),
        ("Which of these pairs of Budapest sights are closest: Parliament and Fisherman's Bastion, or the Opera and Heroes' Square?", "en", "world"),
        ("东方明珠周日几点关门？", "zh", "shanghai"),
        ("上海博物馆需要提前预约吗？", "zh", "shanghai"),
        ("What price level is the restaurant on top of the Shard?", "en", "world"),
        ("Is there parking at the Getty Center, and is it free?", "en", "usa"),
        ("南京路上的第一食品商店评分多少？", "zh", "shanghai"),
        ("Which museum is north of the Tuileries Garden, across the Seine or on the same bank?", "en", "world"),
        ("Is the Alhambra's Nasrid Palace ticket sold at the door?", "en", "world"),
    ],
    "None": [
        ("Is there a lake near Copenhagen's Nørrebro where I could hold a group picnic with a permit?", "en", "world"),
        ("Find me a quiet morning running path from my flat in Kifisia that avoids traffic.", "en", "world"),
        ("Where's the first charging station on my drive from Oslo's Grünerløkka to the airport?", "en", "world"),
        ("Which seafood place in Porto do locals rate best?", "en", "world"),
        ("My car broke down in Gulshan, Dhaka. Which mechanic nearby can I trust?", "en", "world"),
        ("附近有没有靠谱的修车店？我在浦东张江。", "zh", "shanghai"),
        ("What's the best karaoke bar near Hongdae?", "en", "world"),
        ("Is there a dog park near Gràcia where dogs can be off the leash?", "en", "world"),
        ("我在徐家汇，哪家健身房离我近又不太贵？", "zh", "shanghai"),
        ("Which beach near Lisbon is least crowded on weekends?", "en", "world"),
    ],
}


def intent(cls: str, q: str) -> str:
    t = q.lower()
    if cls == "nearby":
        if "second" in t:
            return "the second-nearest place of a given type to a named place"
        if "nearest" in t or "closest" in t:
            return "the nearest place of a given type to a named place"
        if "how many" in t:
            return "how many places of a type are near a named place"
        if "rating" in t or "open" in t:
            return "a nearby place meeting rating or opening-hour conditions"
        return "a suitable nearby place for a stated need"
    if cls == "trip":
        if "order" in t or "optimi" in t:
            return "the best order to visit several places with given visit durations"
        if "what time" in t or "finish" in t:
            return "the finishing time of a multi-stop day trip"
        if "how much time" in t or "how long" in t:
            return "the total time of a multi-leg journey"
        return "planning a multi-stop trip"
    if cls == "routing":
        if "where should i go next" in t or "after reaching" in t:
            return "the next turn on a described route"
        if "how many" in t:
            return "counting features along a route (tolls, merges, turns)"
        if "exit" in t:
            return "which exit to take on a route"
        if "closest" in t or "fastest" in t or "quickest" in t:
            return "which destination is quickest to reach"
        return "a detail of a route between two places"
    if cls == "poi":
        if "farthest" in t or "closest" in t:
            return "which pair of named places is farthest or closest"
        if "open" in t:
            return "a place's opening hours"
        if "reserv" in t or "rating" in t or "price" in t or "accessib" in t or "parking" in t:
            return "a place's attributes (rating, reservations, price, access)"
        if re.search(r"\b(north|south|east|west)\b", t) or "between" in t:
            return "which place lies in a direction from, or between, named places"
        return "a fact about a named place"
    return "a place search the source marks as unanswerable from its data"


def mode(q: str) -> str:
    t = q.lower()
    for words, m in ((("driv", " car", "taxi"), "driving"), (("walk", "on foot"), "walking"), (("bicycl", "cycl", "bike"), "cycling"), (("public transport", "bus", "train", "metro", "subway", "transit"), "public transport")):
        if any(w in t for w in words):
            return m
    return ""


def main() -> None:
    d = json.loads((RAW / "dataset.json").read_text(encoding="utf-8"))
    counters = {k: 0 for k in REQUESTS}
    out = []
    for x in sorted(d, key=lambda x: int(x["id"])):
        cls = str(x["classification"])
        cat, reason = CLASS[cls]
        m = mode(x["question"])
        n = len(x["answer"].get("options") or [])
        summary = f"MapEval-API {cls}: {intent(cls, x['question'])}" + (f", {m}" if m else "") + (f"; multiple choice, {n} options" if n else "") + "."
        pool = REQUESTS[cls]
        text, lang, ctx = pool[counters[cls] % len(pool)]
        counters[cls] += 1
        out.append(
            {
                "source": SOURCE,
                "source_id": str(x["id"]),
                "licence": LICENCE,
                "text_policy": "own_words",
                "summary": summary,
                "category": cat,
                "coverage": "out_of_scope",
                "oos_reason": reason,
                "confidence": "high",
                "notes": "Answered in the source from live place details, nearby search or directions; LayeredMap has no place database or routing. The case request is ours (same question type), not the item's text.",
                "cases": [case(f"mev-{int(x['id']):03d}", text, CANNOT, context=ctx, lang=lang)],
            }
        )
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in out), encoding="utf-8")
    print(f"{OUT.name}: {len(out)} records")


if __name__ == "__main__":
    main()
