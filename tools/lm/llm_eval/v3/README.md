# Harness v3 for the NimbleMap Pro assistant: end to end

v1 and v2 (`../`, `../v2/`) compare the model's tool calls with an expected plan. v3 runs every
request through **the app's own Ask box, headless**, on real files, and checks what the user would
get: the answer's numbers, the objects the map preview highlights, and whether the view frames
them. The results are in `docs/lm/llm-eval.md` › Harness v3 (end-to-end).

## How it works

```
run_v3.py ──job──▶ ask_eval (crates/lm-ui/examples/ask_eval.rs, the app's code)
          ◀─request body─  Workspace::open → ask_inputs → lm_ask::ask (prompt, tools, context,
          ──reply────────▶   resolve_feature, lint, repairs, plan_exec) → Workspace::preview_plan
          ◀─result───────  answer, every step's outcome, what the preview shows
```

- **The app side** is an example binary in `lm-ui` (never shipped). It opens the files as the app
  does, builds the request with `lm_ask` (system prompt, tools, map context, built-in layers from
  `$LM_REFERENCE_DIR`), runs the app's checks, repairs and executor, then calls
  `Workspace::preview_plan` and reads what it puts on the map with `Workspace::preview_shown`
  (the one line added to the app). It never touches the network: each model request goes out on
  stdout and the reply comes back on stdin.
- **This side** decides who answers: OpenRouter (`--mode paid`), the case's gold plan
  (`--mode mock`), or the final plans of an earlier run (`--mode replay`). The key and the
  ledger stay here.

## Files

| File | What |
|---|---|
| `gt.py` | Ground truth with GeoPandas/Shapely/pyproj: geodesic distance to points, lines and areas (azimuthal equidistant around the place, edges densified to 0.02°), point/line in area, the app's join rule (nearest area within 25 km), raster and NetCDF cell values |
| `data_v3.py` | The files and maps, the app's layer names, the derived field names the app makes, object keys |
| `make_cases_v3.py` → `cases_v3.jsonl` | The 166 cases: text, map, gold plan, expected answer and preview, notes |
| `driver.py` | Runs `ask_eval` and passes requests and replies |
| `clients.py` | OpenRouter client (key from the keychain, never printed or stored), plan client (mock/replay), the ledger, the rule-19 ablation |
| `run_v3.py` | The runner; `--estimate` prints the expected cost of a run |
| `scoring_v3.py` | Per case: honest, plan valid (repairs counted), executes, answer correct, preview correct (+ area drawn, frames); one failure class per case |
| `report_v3.py` | `results/summary.{md,csv}`, `results/failures.csv` |
| `test_v3.py` | Unit tests (no network); data and binary tests skip when those are missing |
| `$LM_EVAL_RESULTS/<run>/<model>.jsonl` | Raw results, **outside the repo** (default `/Volumes/LaCie/lm-eval-data/results-v3`, ~220 MB): one line per case, the app's result (answer, steps, outcomes, preview with object keys, the exchange without the system message, usage, latency). No headers, no key |
| `results/scored/<run>__<model>.jsonl` | Per case and run: every score, the failure class, headline, tokens, cost, latency (written by `report_v3.py`) |
| `results/spend.json` | All v3 spending (cap US$10.00) |

## Running

```sh
PY=~/miniforge3/envs/lm-llm-eval/bin/python          # geopandas, shapely, pyproj, xarray, rasterio
CARGO_TARGET_DIR=/Volumes/LaCie/cargo-target/layeredmap-harness cargo build --release -p lm-ui --example ask_eval
$PY -I tools/lm/llm_eval/v3/make_cases_v3.py          # ~10 min (ground truth)
$PY -I -m pytest -q tools/lm/llm_eval/v3
$PY -I tools/lm/llm_eval/v3/run_v3.py --mode mock                     # gold plans through the app
$PY -I tools/lm/llm_eval/v3/run_v3.py --mode mock --tag prefix --bin <pre-fix ask_eval>
$PY -I tools/lm/llm_eval/v3/run_v3.py --estimate --models anthropic/claude-sonnet-5.5
$PY -I tools/lm/llm_eval/v3/run_v3.py --mode paid --tag main --models anthropic/claude-haiku-5.5,z-ai/glm-5.3-flash
$PY -I tools/lm/llm_eval/v3/run_v3.py --mode replay --from results/paid-main/anthropic_claude-haiku-5.5.jsonl --tag prefix --bin <pre-fix ask_eval>
$PY -I tools/lm/llm_eval/v3/report_v3.py
```

The data is outside the repo: `$LM_EVAL_DATA` (default `/Volumes/LaCie/lm-eval-data`, README
there: sources, licences, sizes, checksums) and the app's built-in layers in `$LM_REFERENCE_DIR`.
`gt.py` sets its env's own `PROJ_DATA`/`GDAL_DATA` (a stray `PROJ_LIB` from another install
breaks pyproj).

## Ground truth rules

- "Within N km": geodesic, and a line or area counts when any part is that close (the app's
  buffer + "inside or touching"). **Boundary tolerance 2 km**: objects 2 km either side of the
  limit are *edge* objects; a count is right anywhere between the strict and the loose count,
  and a set must hold every strict object and nothing outside strict ∪ edge.
- In an area: planar in longitude/latitude, the boundary counts.
- Joins: the app's rule (a point just outside every area takes the nearest within 25 km); the
  strictly-inside count is accepted too. A joined line is placed by a point on it; tornado cases
  accept the range over start/middle/end point or touching.
- Nearest: geodesic; every object within 2 km of the nearest is accepted.
- Raster and NetCDF: the cell containing the point; the NetCDF date the map shows is 1 June.
- Names that repeat in World cities (London, Naples, Santiago, Los Angeles, Portland,
  Springfield): the plain reading is planned, asking is accepted where listed.
- Questions about a date the grid doesn't show expect `cannot_do`/`ask_user` (no tool picks a
  date): those cases are tagged `tool_gap`.

## Scoring and classes

See `scoring_v3.py`. The gold plan's own run tells the app's faults from the model's: a wrong
answer the gold plan shares is an executor fault (or ground-truth ambiguity for cases tagged
`gt_rule`); a runtime error the gold plan shares is an executor fault; a "no field" error on a
made field is a prompt-tool gap; a right answer with a wrong preview is preview-UI.
