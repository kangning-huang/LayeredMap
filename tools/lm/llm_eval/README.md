# LLM evaluation for LayeredMap Pro

Which OpenRouter model turns a user's request (English or Chinese) into calls to LayeredMap's
certified actions most cost-efficiently? The model only translates: it picks actions and fills
their inputs, and LayeredMap runs them locally (spec › Product tiers, Phase 7). The report is
[`docs/lm/llm-eval.md`](../../../docs/lm/llm-eval.md).

## Files

| File | What |
|---|---|
| `tools.py` | Builds the tool definitions from the app's sources (below); writes `tools_per_action.json` and `tools_single.json` |
| `context.py` | The map context (system message): instruction, open layers, selection. Value-free by default |
| `make_cases.py` → `cases.jsonl` | The 78 benchmark cases, written by hand |
| `scoring.py` | Strict and lenient scoring |
| `run_eval.py` | Calls OpenRouter; records calls, scores, tokens, cost, latency |
| `report.py` | Summary CSV and markdown, failure tables, the Pareto chart |
| `test_scoring.py` | Unit tests (scoring, tool generation, the runner with a mocked client) |
| `tokens.py` | Offline token estimates (tiktoken o200k_base) |
| `openrouter_models_2026-10-17.json` | OpenRouter's public model list, fetched 2026-10-17: prices, parameters, reasoning settings |
| `results/<date>/<run>/<model>.jsonl` | One line per case: the calls made, both scores, tokens, cost, latency, provider. No headers, no key |
| `results/<date>/summary.{csv,md}`, `spend.json` | Summaries and the money spent that day |

## Running

```sh
mamba create -n lm-llm-eval python=3.12 requests matplotlib pandas pytest tiktoken -c conda-forge -y
PY=~/miniforge3/envs/lm-llm-eval/bin/python
$PY -I tools/lm/llm_eval/tools.py                     # regenerate tools_*.json, print sizes
$PY -I tools/lm/llm_eval/context.py                   # context sizes
$PY -I tools/lm/llm_eval/make_cases.py                # regenerate cases.jsonl
$PY -I -m pytest tools/lm/llm_eval/test_scoring.py -q -p no:cacheprovider
$PY -I tools/lm/llm_eval/run_eval.py --dry-run        # mocked client: no key, no network
$PY -I tools/lm/llm_eval/run_eval.py                  # the 15-model shortlist, design A
$PY -I tools/lm/llm_eval/run_eval.py --design single --models <ids>
$PY -I tools/lm/llm_eval/run_eval.py --variant with_values --models <ids>
$PY -I tools/lm/llm_eval/run_eval.py --tag repeat2 --models <ids>   # a repeat, in its own folder
$PY -I tools/lm/llm_eval/report.py                    # rescore every run, write summaries and the chart
```

Other options: `--provider-ignore Darkbloom` (skip a failing OpenRouter provider), `--prompt v2`,
`--only a01,m02`, `--rerun`, `--budget`, `--concurrency`.

The key is read at run time from the macOS keychain
(`security find-generic-password -s openrouter -a layeredmap -w`) and is never printed, logged or
stored. Only the synthetic benchmark text is sent. `--budget` (default US$3) caps everything
spent on one date (`results/<date>/spend.json`); before each request the runner reserves an
upper estimate of its cost and stops when the cap would be passed. Requests run concurrently
(`--concurrency 8`, `--per-model 4`) and are retried with exponential backoff on 429, 5xx and
network errors (up to 6 tries). A run resumes: cases already scored are skipped.

## Request settings

`temperature 0` where the model accepts it (OpenAI's GPT-5 family and Anthropic's models on
OpenRouter don't list it); `usage: {include: true}` for the real cost; `max_tokens 4000`;
`tool_choice auto`. Reasoning is set to the lowest the catalog allows: switched off where it is
optional (`reasoning: {enabled: false}`), otherwise the lowest listed effort (`minimal` or
`low`). Reasoning tokens are recorded.

**Turns.** The instruction asks for the whole plan, and says nothing runs until the user approves
it. Most models still call one tool per turn and wait for its result, so each case is a short
loop: every tool call gets a stub result ("Step #k added to the plan (not run yet)", plus the new
layer's `#k` and field names, which the app knows before running), and the model may continue,
up to 5 turns, until it replies without a call or calls `ask_user` / `cannot_do`. The plan is all
calls in order. Tokens, cost and latency are summed over the turns, so a model that needs an
extra turn to say "done" pays for it. No real result or feature value is ever sent back.

**Calibration.** Before the full run, a smoke test (6 cases × 5 models, US$0.05) showed that
single-turn plans dropped every second step and that models guessed the count field's name;
the loop above, the "nothing runs until approved" sentence and an example field name in the
tool notes ("Schools: 'schools'") were added then. After the full run, a second instruction
(`--prompt v2`: "your calls are the plan; never a text plan") was tried on 12 cases × 4 models
because GPT-5 models wrote plans as text; it made no material difference, so `v1` stays the
default. Review of the full run also added one strict alternative (case a11) and three lenient
rules (exact repeats, `#k` pointing at a changed layer, Gemini's `default_api.` prefix); every
run is rescored from its stored calls by `report.py`, so all results use the same rules.

## Tool interface

Generated from the app so it stays in sync:

- the connected actions: `ACTIONS` in `crates/lm-jobs/src/actions.rs` (13 actions;
  `measure_from_here` is in `rules.toml` but not connected, so it is not offered);
- each action's inputs, in the runner's order: the module-doc table in the same file; each input
  becomes a named argument (`points`, `areas`, `raster`, `layer`: a layer's display name;
  `point`, `area`, `line`, `object`: one object as `{layer, field, value}`; omitting field and
  value means the selected object or a drawn area);
- parameters: `distance` ("500 m", "2 km", "3 mi", the runner's parser), `merge`, `band`, `field`;
- descriptions: the action's label from `rules.toml` with tokens replaced, plus the first clause
  of its `method`, plus a note on what it makes (written from the runner; a test checks the
  runner still names outputs that way).

Display tools: `filter(layer, conditions[{field, op, value, value2, values}])` with the ten
conditions of `lm-core/src/filter.rs`; `color_by(layer, field)`; `view_data(layer)`;
`zoom_to(layer, field?, value?)`. Meta tools: `ask_user(question, choices)` and
`cannot_do(reason)`. A later call can use an earlier call's new layer as `"#n"`.

Two designs: (A) `per_action`, one tool per action (19 tools); (B) `single`, one
`run_action(action, layers[], object, distance, merge, band, field)` with an enum and the action
list in its description (7 tools).

## How the cases were made

All 78 cases were written by hand in `make_cases.py`, with the expected plan worked out from the
runner's input order and the fields of the real files:

- **world** context: `assets/examples` (Countries 177 areas, Populated places 1,251 points,
  Elevation raster in metres);
- **shanghai** context: `fixtures/V1/districts.geojson` (Districts), `fixtures/T1/schools.csv`
  (Schools, 242 located), the roads and parks of `fixtures/V9/city_layers.gpkg`, and two rasters
  described as the app would describe them (Surface temperature in °C, Land cover categories);
  **shanghai_drawn** adds a drawn area "Area 1".

Every object named in a case was checked against the data (e.g. Timbuktu is in Populated places;
Century Ave is road 2), and each expected object lists every (field, value) pair that names it
(`name` France, `name_zh` 法国, `iso_a3` FRA). Categories: every connected action (25 cases plus
more in multi-step plans), filters and styling, multi-step plans (11), Chinese (17), vague
requests that should ask (5), impossible ones that should be refused (6), wrong-layer traps (7),
and unit conversions (11: miles, feet, hectares, 公里, 米, million US$, 一亿). Cases were not tuned
on any model's output; the one later change (an alternative plan for a11) is noted above.

## Scoring

Each reply's tool calls are mapped to canonical calls (design B's `run_action` is unpacked by the
same input table), then compared with the expected plan and its `alts` (other plans that are
equally right, e.g. filtering before or after adding raster values). Only the arguments listed in
the expected call are checked, except that buffering one object when the whole layer was meant
fails.

**Strict** (could LayeredMap run it as is?): the same calls in the same order and nothing more;
layer and field names exactly as listed; a result layer as the right `#n` or its real name;
objects named by a (field, value) pair that exists in the data (case-insensitive value, as the
app's filter compares text); distances the runner's `parse_distance` reads, within 0.5 %; filter
operators exactly as defined, numbers parseable, within 1e-9 (1 % for 5,000 ft = 1,524 m);
`capital is true` or `yes`. `ask_user` and `cannot_do` are scored on the call alone.

**Lenient** (could a forgiving plan checker repair it?): also accepts names that differ only in
case or spacing; any `#n` for a result layer; objects matched on the value alone, plus a few
aliases (Century Avenue, 世纪大道); distances in Chinese units (米, 公里) or as numbers, within 2 %;
operator synonyms (`>`, `gt`, `in`…), `is X` = `any of [X]`, `above a` + `below b` = `between`;
numbers with thousands separators; consecutive filters on one layer merged; extra `zoom_to`,
`view_data` or `color_by` calls and exact repeats of a call ignored; a `#k` that points at a
filter or add-values step read as the layer that step changed; JSON arguments sent as strings decoded; a plain-text
reply with a question mark counts as `ask_user`, and a plain-text reply as `cannot_do`; and the
case's `lenient_alts` (acceptable but not ideal plans, e.g. a buffer with the default distance
when none was given).

Accuracy is over all 78 cases; a request that failed after retries counts as wrong.
