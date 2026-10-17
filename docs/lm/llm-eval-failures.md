# Why the assistant's plans fail: analysis of the 2026-10-17 runs

Source: `tools/lm/llm_eval/results/2026-10-17/`. The six leading models (Haiku 5.5 run twice, Sonnet 5.5, GLM 5.3 Flash, gpt-oss-120b and Gemini 3.5 Flash) made 468 runs with 94 strict failures. Prior work: `docs/lm/llm-harness-research.md`.

| Cause | Misses | Typical case | Fix (harness v2) |
|---|---:|---|---|
| Extra steps nobody asked for | 28 | "view data" or "zoom" after a filter; counting both before and after filtering | A rule in the instructions plus worked examples; the plan checker drops trailing display-only steps (zoom, view data, colour) the request doesn't mention |
| Wrong object name | 19 | 世纪大道 / 延安路 passed as written, while the data stores "Century Ave" / "Yan'an Rd" | Object references carry the original text and an English form; the app resolves names locally (or, if the owner allows, a `resolve_feature` lookup tool) |
| No tool call: the plan is written as text or JSON | 11 | gpt-oss-120b writes `{"tool": "buffer", …}` as text (8 cases); GPT-5 models write prose | Force a tool call (`tool_choice: required`); parse JSON written as text as a fallback; a single `submit_plan` tool, tested A/B |
| Wrong layer | 9 | Colours "Countries" instead of the new count layer | Rule plus example: a step's output is "#1", "#2"…; the checker suggests the output layer |
| Wrong filter value | 8 | "高中" instead of "High"; "yes" instead of "true"; value missing | The app matches the value to the field's categories locally; the checker returns specific errors ("value missing", "not a category of type: High, Middle, Primary…" — names of categories only if the owner allows) and the model repairs (up to 2 rounds) |
| Wrong action | 5 | `summarize_data_here` for a raster; filtering the input layer instead of the result | Clearer tool descriptions; the checker refuses a raster in a vector-only slot |
| Ask / act / refuse mix-ups | 14 | A refusal written as text instead of a `cannot_do` call; asking what "land cover shares" means | Accept a refusal written as text; the summary tool's description says category rasters give shares |

## Benchmark corrections before re-measuring

- **x05** "total number of students in each district" is now answerable: count in areas with the statistic sum of `students`. Its expected plan changes from `cannot_do` to that.
- **m06** "Rank the African countries by average elevation": filtering after the summary is as correct as filtering before. Add it as an alternative.
- Add cases for the new statistic menus (median elevation, mean of a point field), and keep repeat runs: each case is about 1.3 points, and runs moved by up to 7 points.

## What the numbers suggest

Most misses are harness and benchmark problems, not model ability:
- names and category values the model can't see;
- missing feedback;
- format;
- unclear tool descriptions.

In the CRITIC paper, external feedback gained 7–8 points, and LangChain measured large gains for Haiku from a few examples (`llm-harness-research.md`). On our cases, a checker with repair plus local name and value resolution should address roughly 60 of the 94 misses. The re-run will show whether it does.
