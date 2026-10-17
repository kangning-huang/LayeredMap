# Harness v2 for the LayeredMap Pro assistant

v2 sits beside v1 (`../`), which is unchanged and still runs (`../run_eval.py`, `../cases.jsonl`).
The plan for v2 came from `docs/lm/llm-eval-failures.md`; the results are in `docs/lm/llm-eval.md`
› Harness v2.

## Files

| File | What |
|---|---|
| `data.py` | Attribute tables of the benchmark layers, read from the fixture files (local only) |
| `resolve.py` | `resolve_feature(layer, text)`, simulated: normalized, alias, base-name, pinyin and fuzzy matching; the zh→en alias table |
| `tools_v2.py` | Tools per map context: layer enums per slot, `stat` menus, `resolve_feature`; designs `per_action` and `submit_plan` |
| `prompt_v2.py` | Instructions (10 rules) and 9 worked examples on an invented map; system message with `cache_control` |
| `checker.py` | The deterministic plan checker: per-step checks with corrections, plan-level lint, trailing display drop |
| `make_cases_v2.py` → `cases_v2.jsonl` | The 97-case v2 benchmark (78 from v1 with 2 corrections, 19 new) |
| `scoring_v2.py` | Two scoring modes: `v1` (the original 78 with v1's cases and rules) and `v2` (97 cases, statistic checked) |
| `run_v2.py` | The runner (OpenRouter), the text-plan fallback parser, the mock client |
| `report_v2.py` | Summary of the v2 runs beside the v1 baseline → `results/summary.{md,csv}` |
| `test_v2.py` | Unit tests (resolver, checker, scoring modes, parser, leakage, dry runs) |
| `results/<run>/<model>.jsonl` | One line per case: raw calls per turn, checker log, final plan, dropped steps, both scores, tokens (incl. cached), cost, latency, rounds. No headers, no key |
| `results/spend.json` | Money spent on all v2 runs (cap US$3.00, separate from v1's ledger) |

## Running

```sh
PY=~/miniforge3/envs/lm-llm-eval/bin/python      # needs pypinyin (conda-forge) for pinyin matching
$PY -I tools/lm/llm_eval/v2/make_cases_v2.py
$PY -I -m pytest tools/lm/llm_eval/v2/test_v2.py -q -p no:cacheprovider
$PY -I tools/lm/llm_eval/v2/run_v2.py --dry-run
$PY -I tools/lm/llm_eval/v2/run_v2.py --models anthropic/claude-haiku-5.5,z-ai/glm-5.3-flash,openai/gpt-oss-120b
$PY -I tools/lm/llm_eval/v2/run_v2.py --tag repeat2 --models ...
$PY -I tools/lm/llm_eval/v2/run_v2.py --design submit_plan --models ...
$PY -I tools/lm/llm_eval/v2/run_v2.py --no-examples --models anthropic/claude-haiku-5.5   # ablations:
$PY -I tools/lm/llm_eval/v2/run_v2.py --no-checker  --models anthropic/claude-haiku-5.5   #   --no-resolve, --auto, --no-cache
$PY -I tools/lm/llm_eval/v2/report_v2.py
```

The key handling is v1's: read from the keychain at run time, never printed, logged or stored.

## Benchmark changes (v1 → v2)

All in `make_cases_v2.py`; `../cases.jsonl` is untouched.

| Change | Why |
|---|---|
| **x05** "total number of students in each district": `cannot_do` → `count_in(Schools, Districts, stat=sum, field=students)`; category impossible → stat | The runner now offers a statistic of a numeric point field |
| **m06** "Rank the African countries by their average elevation": strict alternative `summarize_each` → `filter(#1, continent is Africa)` → `rank_by_value(#1, 'Elevation mean')` | Filtering the summary layer is as right as filtering Countries first |
| Every expected `count_in` / `summarize_*` call states its `stat` (`count` / `mean` by default) | A plan asking for another statistic must fail; a model's call without `stat` is read as the default |
| 10 new statistic cases `s01`–`s10` (3 Chinese): mean / median / max of `students` per district, sum of city population per country, median / max / min / std of a raster per area, max in the selected district, median in a drawn area | The new menus |
| 9 new name/category cases `r01`–`r09` (6 Chinese): 南京路, 初中, 东京, 浦东新区, Huaihai Road, 内环, 城市公园, "high schools in Pudong", and **Portland** (two places: should ask) | `resolve_feature` |

97 cases, 26 in Chinese. The 19 new cases were written before any v2 run and not changed after.

**v1-compatible scoring.** `scoring_v2.score_both` scores every final plan twice: against the
original 78 cases with v1's `scoring.score` (so x05 still expects `cannot_do` and `stat` isn't
checked), and against the 97 v2 cases. The v1 numbers of the old runs and the v1-mode numbers of the
new runs are therefore directly comparable.

## Design

**Tools** (`tools_v2.py`). Built per map context. Layer arguments are enums of the open layers by
slot kind: a raster slot lists only rasters, a points slot only point layers, an areas slot area
layers, drawn areas and `#1`…`#6`; `additionalProperties: false`; an empty enum (no line layer on the
world map) becomes a plain string, because one provider (Inceptron, serving GLM) rejected the whole
request for it. `strict: true` is not set: strict mode needs every property required, so optional
inputs would become nullable unions. `count_in` takes `stat` and `field`; `summarize_*` take `stat`;
descriptions say what each step makes ('sum students', 'Elevation minimum', category rasters give
shares) and when not to use the display tools. `submit_plan(steps[])` has one flat step object
`{action, ...named inputs}`.

**resolve_feature** (`resolve.py`). Searches the layer's text, category and yes/no fields (so
`name_zh` too) and returns up to 5 stored values with their field and how many objects carry each.
Matching: exact; case/punctuation-folded; the alias table; base names (abbreviations expanded,
generic words dropped: "Huaihai Road" = "Huaihai Rd", 浦东 = 浦东新区); pinyin of a Chinese query
against an English name (南京路 → nanjing = "Nanjing Rd"; pypinyin); inflected forms (African →
Africa); prefix/substring; fuzzy (difflib ≥ 0.8). The **alias table** stands in for a Wikidata /
GeoNames zh–en label lookup and covers only the benchmark: the six roads (延安路, 世纪大道, 南京路,
淮海路, 中山路, 内环), school types (高中, 初中, 中学 = Middle or High, 小学), park kinds (城市公园, 区级公园,
口袋公园), continents (亚洲 …) and capital (首都, 是/否). The tool result names candidates only, says
"(a category)" or "(N objects share this name)", and tells the model to ask when several objects
could be meant.

**Plan checker** (`checker.py`), run on every planned step before it is added:
- layers exist and fit the slot; `#k` refers to an earlier step that makes a layer (a `#k` naming a
  filter or add-values step is replaced by the layer that step changed);
- required inputs are present; fields exist on the layer at that point of the plan, including fields
  that earlier steps add; rank fields and statistic fields are numeric;
- distances parse with the runner's parser (Chinese units and bare numbers are rewritten as metres);
- objects name exactly one stored object, and filter values match stored values or categories. A
  unique, confident match through `resolve.py` is corrected in place and reported ("Corrected: …");
  otherwise the error names the candidates, and several objects with one name says to ask;
- `before`/`after` only on dates; `stat` on the action's menu, with a numeric `field` for `count_in`;
- a step identical to an accepted one is not added again (no error);
- heuristics, both keyed on the request text: a filter with no conditions (clears the filter) is an
  error unless the request says clear/remove/all/清除/取消/全部…; a buffer of a layer that has a
  selection gets the selected object when the request says selected/this/these/选中/这个.

Errors go back as that call's tool result ("Not added. …") for up to **2 repair rounds**; after that a
failing step enters the plan as given. When the model stops, a plan-level **lint** checks data flow:
a filter or add-values step whose layer no later analysis step uses (e.g. filtering Countries after
`summarize_each` already made `#1` from it) is reported, the plan is emptied and the model re-plans
(this uses a repair round). Finally, trailing `zoom_to` / `view_data` / `color_by` steps are dropped
unless the request has one of their words (zoom, table, attribute, colo(u)r, style, 缩放, 定位, 表格,
属性表, 上色, 颜色 …) or dropping would leave the plan empty.

**Prompt** (`prompt_v2.py`). Ten explicit rules (only the needed steps; `#k` numbering, lookups are
not steps; filter first; resolve names; selected objects; units; statistics; refuse with a
`cannot_do` call; ask only when needed; repair). Then **9 static worked examples** (3 Chinese) on an
invented map (Hospitals, Wards, Rivers, Rainfall, Soil type) that shares no layer, field, object or
request with the benchmark; `test_v2.py` checks that the prompt names no benchmark layer and that no
example request is near any test request (character-trigram Jaccard < 0.35; two examples were
reworded to pass). Static rather than retrieved, so the whole prefix is cacheable.

**Caching.** For Anthropic and Google models the system message is two text parts: the static
instructions + examples with `cache_control: {type: ephemeral}`, then the map context. Tools come
first in the cache order, so the cached prefix is tools + instructions (one entry per map context).
Cached and cache-write tokens are recorded from `usage.prompt_tokens_details`.

**Format.** `tool_choice: "required"` while the plan is still empty (a lookup alone doesn't count),
then `auto` so the model can stop; always `required` for `submit_plan`. `provider:
{require_parameters: true}`. No Anthropic endpoint on OpenRouter accepted a forced tool choice (404
"No endpoints found that support the provided 'tool_choice' value"), so Haiku falls back to `auto`
(learned once per run); for GLM only one provider (Inceptron) accepts it. If a reply has no tool
call, `calls_from_text` parses JSON plans in the shapes models write, `functions.name {…}`, and the
compact notation of the worked examples; a prose reply with no plan becomes `ask_user` if it asks a
question, else `cannot_do` (`text_as_meta` in the results).

## Calibration (what changed after seeing runs)

- **Smoke test** (8 cases × 3 models × 2 designs, US$0.03): found the empty-enum schema error
  (Inceptron), the missing forced tool choice for Anthropic, and the resolver ranking "Central
  African Rep." above "Africa" for "African" (added the inflected-form rule).
- **Development run 1** (`results/v2-per_action-dev`, full, 3 models): gpt-oss-120b wrote the plan as
  text after a lookup in 7 cases, and Haiku once echoed the examples' notation as text. Changes:
  `required` until the plan has a step, the wider text parser, prose replies read as ask/refuse, the
  data-flow lint (a GLM m06 plan filtered Countries after summarizing it), the empty-filter check
  (gpt-oss r08), the selected-object rule for buffers (u04 failed for all three models).
- **Development run 2** (`results/v2-per_action-dev2`): u04 still failed for all three (they buffer
  the layer and say "around the selected city"), so the checker's selection rule for buffers was
  added; `before`/`after` on a number became an error (Haiku f17).
- The reported runs used the frozen harness. Development runs are kept but excluded from the summary.
  The selection rule and the lint were motivated by single cases (u04, m06), so they flatter those two
  cases; both are general app rules, but count them as tuned.
