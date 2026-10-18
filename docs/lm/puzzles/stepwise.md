# The eight puzzles, one request at a time

2026-10-18 · `tools/lm/puzzles/stepwise.py`, requests in `tools/lm/puzzles/steps.json` · model: Claude Haiku 5.5 (`submit_plan` design, plan checker, `resolve_feature`)

The owner's direction (17 Oct): Pro's assistant is judged on single requests a non-expert would make, and a user may break a puzzle into steps. So each puzzle was written as the requests a user would type one after another, 57 in all ("Show only the cities in China with more than two million people", "Add the elevation to each of these cities", "Keep only the ones at least 1,500 metres high"…). The wording is ours. Each request names what the user wants in everyday words, never a field name.

**In short.** In the final run Haiku carried out **all 57 requests correctly**. Following its own plans from start to finish, it reached the answer of **7 of the 8 puzzles**. The run cost **US$0.067** (about US$0.0006 a request). Getting there took harness fixes (from 51 of 57) and two general rules. Three of the fixes are things the app's assistant needs too (section 4).

## 1. How it was tested

Each request was asked twice:

- **Guided.** The map is in the state that the gold steps of the earlier requests leave it in. The request counts as right when its plan changes the map as the gold steps do, checked by running both with `run_plan` on the real data. The check compares:
  - the objects shown in every layer (how many, and the first ones);
  - the new layers, by size;
  - the value summaries of added fields;
  - the answers on cards (a layer of the same objects also counts, e.g. keep_inside for "which countries are within 30 km").

  Extra harmless steps are allowed. This measures single requests on their own.
- **On its own.** The map is in the state Haiku's own earlier plans left it in, as in the app. At the end, the puzzle's answer is checked as in the end-to-end report (`e2e.md`). This measures whether a user who types these requests gets the answer.

For each request, the model sees:
- the open layers and the layers made in earlier requests, by name;
- each layer's fields, including the ones added earlier;
- the filter on each layer and how many objects it shows ("Filter now: CONTINENT is Africa; POP_MAX at least 1000000 (53 of 7,342 shown)");
- the earlier requests.

As before, it sees no feature values, except what `resolve_feature` returns for the user's words.

## 2. Results

| Run | What changed | Requests right (guided) | Puzzles solved on its own | Cost |
| --- | --- | --- | --- | --- |
| 1 | — | 51 of 57 (89%) | 5 of 8 | US$0.071 |
| 2 | The checker knows each layer's filter; joined categories can be looked up; "tropical" and similar terms defined; a layer's only shown object can be named by the layer | 52 of 57 (91%) | 6 of 8 | US$0.077 |
| 3 | Joined text fields can be looked up; the checker accepts a layer's only shown object; fairer comparison | **57 of 57 (100%)** | **7 of 8** | US$0.067 |

Run 1 is shown as rescored with run 3's comparison. The fixes were made after looking at these same 57 requests, so run 3 is an upper bound for requests like these, not a held-out score.

Final run, per puzzle:

| Puzzle | Requests | Right | Answer reached on its own |
| --- | --- | --- | --- |
| P1 | 5 | 5 | yes |
| P2 | 4 | 4 | yes |
| P3 | 6 | 6 | yes |
| P4 | 5 | 5 | yes (area and town) |
| P5 | 4 | 4 | yes |
| P6 | 11 | 11 | yes |
| P7 | 11 | 11 | no (see below) |
| P8 | 11 | 11 | yes |

**P7, on its own.** For the last request, "And which countries are within 30 km of Guaira?", Haiku drew the circle and then counted *cities* in it (count_in) instead of listing the countries inside. The same request in the guided run got the right plan. It's an inconsistency, not a missing capability. A recipe preview of "count populated places in the area" would let the user notice before running it.

## 3. What went wrong before the fixes

| Request | What happened | Cause | Fix |
| --- | --- | --- | --- |
| P2.2 "North American cities…" (first try) | Asked a question: "CONTINENT holds numbers" | Harness: the joined field was labelled a number | Harness: a joined field keeps its type from the area layer |
| P3.5 "South Korean islands between 50 and 100 km²" | Asked a question twice: "couldn't find South Korea" | `resolve_feature` and the checker read the islands' own values, which don't include the joined ADMIN | Lookups and checks of a joined field use the values of the layer it came from |
| P3.6 "Which of those are at least 75 km from any other land?" | Filtered by distance only, dropping "South Korea" and the area range | The model didn't repeat the conditions already on the layer | The checker knows each layer's current filter and sends the plan back once when it drops one, unless the user asked to show all |
| P4.5 "Which towns are in it?" | Asked "which area?" although the filter leaves one area | The checker rejected `{layer}` with nothing selected | `{layer}` alone names the layer's only shown object, as the runner already does; stated in the rules and the checker |
| P8.4 "tropical islands of 50 to 150 km²" | Asked whether tropical means ±23.5° | No rule for standard terms | Rule 14: tropical, hemispheres, Arctic and Antarctic by their standard latitudes, without asking |
| P3.4, P7.10, P7.11 | Judged wrong, but right for the user: joined more fields than asked, or answered "which countries" with a layer of them | Comparison too strict | Comparison allows extra fields and extra layers, and a layer as a list's answer |

## 4. What this means for the app

The harness is the prototype of Pro's assistant (WP7.2). Three of the fixes are things the app must do too:

1. **The plan checker gets the map's current state:** each layer's filter conditions and which layers show exactly one object. "Keep those…", "Of these…" and "in it" depend on it.
2. **`resolve_feature` and value checks cover joined fields,** using the values of the area layer they came from (ADMIN on islands joined from Countries).
3. **The assistant's context lists what earlier steps did:** fields added, new layers by name, and filters with counts.

The rules added (current filter, the only shown object, standard geographic terms) are in `tools/lm/llm_eval/v2/prompt_v2.py`, `tools_v2.py` and the puzzle rules in `pro_e2e.py`.

## 5. Running it

```sh
cargo build -p lm-jobs --release --example run_plan
mamba run -n lm-llm-eval python -I tools/lm/puzzles/stepwise.py --gold                  # gold steps: all 8 solved
mamba run -n lm-llm-eval python -I tools/lm/puzzles/stepwise.py --show-context P6 7     # what the model sees
mamba run -n lm-llm-eval python -I tools/lm/puzzles/stepwise.py --models anthropic/claude-haiku-5.5 --budget 0.2 --tag haiku-3
```

Results: `tools/lm/puzzles/results/stepwise-haiku`, `-haiku-2` and `-haiku-3` (plans, checker logs, cost, verdicts; `summary.md` in each). The three runs together cost US$0.23. All puzzle-harness spending is US$0.58 (`results/spend.json`).
