# Demo and tutorial picks: a map enthusiast's ranking

2026-10-18. Judged by a sub-agent persona: "Mei", a map enthusiast with no GIS training. The owner asked which of the successfully tested assistant examples would excite lay people most, for demos and the tutorial of the Ask box. Sources: the v2 single-request evaluation (Claude Haiku 5.5, `submit_plan`; `tools/lm/llm_eval/v2/results/`) and the step-by-step puzzles (`docs/lm/puzzles/stepwise.md`). The bundled example layer `populated_places` was renamed `cities` the same day; the requests below read "cities" accordingly.

---

# Mei's picks: the examples most likely to excite people who don't use GIS

*Mei: "I don't want to make a map. I want a map to tell me a secret."*

Everything below passed testing. The single requests come from the Claude Haiku 5.5 submit_plan run in `tools/lm/llm_eval/v2/results/v2-submit_plan/`, which got 95 of 97 cases right; the two it missed were f13 and m10. The puzzles come from `stepwise-haiku-3`, where all 57 requests were right. One limit applies to the single requests: they were scored on whether the plan was correct, and the eval did not check what appears on the map. To get a feel for the results, I sampled the bundled elevation file at each city myself.

## Tutorial starters (one request, bundled data)

**1. "Which capital city sits highest above sea level?"** (m04, bundled data)
Mei: *"La Paz! I knew it, but seeing it pop out first is so satisfying."*
Why it works: it's a trivia question with a dramatic answer. In my check the top five were La Paz (about 4,265 m), Sucre, Mexico City, Thimphu and Addis Ababa. The app adds elevation to every city, keeps only the capitals and ranks them, so you watch three steps happen from one sentence.

**2. "Show the cities higher than 5,000 feet."** (u03, bundled data)
Mei: *"Wait, there are towns in Antarctica up there?"*
Why it works: about 83 dots light up across the Andes, Tibet, Ethiopia and Mexico, so the mountain belts appear on their own. Leh, Lhasa and Cusco are near the top, plus two Antarctic research stations as a surprise. It also shows that feet work as well as metres.

**3. "Show everything within 100 miles of Paris."** (a11, bundled data)
Mei: *"Ooh, a circle! I'd do this with my hometown."*
Why it works: one big, instantly readable shape, and an obvious "now try your own city" invitation.

**4. "Color the countries by GDP."** (f08, bundled data)
Mei: *"It's an atlas page!"*
Why it works: the whole world map changes colour in one go, and it looks like the maps she already loves.

**5. "Which country is Timbuktu in?"** (a05, bundled data)
Mei: *"Timbuktu is real?! And it's in Mali!"*
Why it works: it settles a famous "middle of nowhere" place, and the answer is that its country gets highlighted.

## Showpieces (multi-step stories, extra data needed)

These need the puzzle data: WorldClim climate, Natural Earth land, coastline and rivers, and the active faults map.

**6. P8, The Red Crab Crossing** (11 requests, solved)
Ends with: "Keep the islands whose ratio squared is between 90 and 110."
Mei: *"Millions of red crabs AND a riddle about longitude? Christmas Island! I'm obsessed."*
Why it works: it has the best story and the most magical ending. Thousands of islands shrink to 346, then 5, then 2, then 1. The last three requests are arithmetic, though (see the dull list), so cut them in the edit or show them on a card.

**7. P3, Grandpa's Squid Boat** (6 requests, solved)
"Split the land into separate islands." … "Which of those are at least 75 km from any other land?"
Mei: *"A lonely volcanic island full of squid lamps. That's a movie."*
Why it works: it's short and emotional. Six islands become one, Ulleungdo, alone in the East Sea.

**8. P6, The Running Club's Secret Hill** (11 requests, solved)
Ends with: "Which one has an active fault within 50 km?"
Mei: *"Runners training above a crack in the Earth: Nairobi!"*
Why it works: the twist is that Johannesburg and Harare survive every clue until the very last one, the fault line, knocks them out.

**9. P1, Grandma's Flower Postcard** (5 requests, solved)
"Show only the cities in China with more than two million people." → "Add the elevation to each of these cities." → "Keep only the ones at least 1,500 metres high." → "What's the January average temperature at each of them?" → "Which of them have a January average of 5 °C or warmer?"
Mei: *"Kunming, the Spring City! It's short enough for a 30-second ad."*
Why it works: the cities go 45 → 2 → 1, which is easy to follow. Only the January temperature needs extra data; the first three steps run on the bundled data.

## Fun with Chinese

**10. "只显示人口超过1000万的城市"** ("Show only cities with more than 10 million people", t06, bundled data). Also "东京在哪个国家？" ("Which country is Tokyo in?", r04).
Mei: *"I typed it in Chinese and the megacities just appeared!"*
Why it works: it's a world map of the giant cities, from a question asked in Chinese. The bundled data has Chinese names (`name_zh`), and the P1, P3 and P8 puzzle stories also come in Chinese. One caution: the Chinese versions of the puzzles were never run step by step; only the English requests were.

**Honourable mention:** P2, Aurora Week (Winnipeg is the only North American city of 500,000+ with a January of −15 °C or colder). It has a fun reveal but is a bit plain.

## Examples Mei found dull or confusing

- **Anything about the lowest elevation** (s07, "Rank the countries by their lowest elevation"). It passed, but on the bundled data each value is an average over a cell about 50 km wide, so sea floor leaks in. In my check, island capitals like Melekeok (Palau) came out about 4,000 m *below* sea level. Viewers would think the app is broken.
- **Spreadsheet-style requests**: "Open the attribute table of the cities," "Show countries whose population estimate is older than 2019," "Show the countries with no GDP figure." *"That's homework, not geography."*
- **Statistics jargon** like median or standard deviation (s04, s06). The words alone scare people off.
- **P8's last three steps** ("Divide each island's longitude by its latitude and call it ratio," "ratio squared"). Great answer, maths-class wording.
- **P7**: the assistant didn't solve it following its own plans, so don't use it in a demo.
- **The Shanghai cases** (schools, districts, roads). They passed, but they use test data that doesn't ship with the app, and "500 m around each school" feels like a planning office.
- Skip f13 and m10, which failed, and the vague or refusal cases.

## Phrasing that makes a request feel exciting

1. **Ask for a winner, not a filter.** "Which capital sits highest?" beats "filter capitals and rank by elevation." Superlatives (highest, coldest, loneliest) promise a reveal.
2. **Use people, places and everyday units.** "Within 100 miles of Paris" or "towns higher than 5,000 feet" beats "buffer" or "above 1524". Name somewhere famous like Timbuktu, Tokyo or La Paz.
3. **Wrap the multi-step demos in a story.** Grandma's postcard, crabs, runners. Then each step reads as a clue being crossed off, and the shrinking count of dots (45 → 2 → 1) carries the tension.
